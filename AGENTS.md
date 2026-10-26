# AGENTS.md

Instructions and maintenance guidelines for AI coding agents working in the `cosmic-applet-unsplash` repository.

---

## 1. Project Overview

`cosmic-applet-unsplash` (`io.github.cosmic.AppletUnsplash`) is a native panel applet for the **System76 COSMIC Desktop Environment** built with `libcosmic` (`iced` + Wayland `xdg_popup`), `tokio`, and `reqwest` (`rustls-tls`). It fetches random photos from the Unsplash API (`GET /photos/random`) using user-configured search and filter parameters and applies them to the desktop via `cosmic-bg`.

### Key Modules

- [`src/main.rs`](src/main.rs): Application entry point supporting three execution modes:
  1. Default panel applet mode (`cosmic::applet::run::<UnsplashApplet>(false)`)
  2. Standalone desktop window mode (`--window` / `-w`)
  3. Headless CLI one-shot mode (`--fetch-now`, `--save`, `--access-key`, `--query`, etc.)
- [`src/error.rs`](src/error.rs): Structured `AppError` enum (`thiserror`) covering HTTP, JSON, RON, I/O, and CLI validation errors.
- [`src/window.rs`](src/window.rs): `cosmic::Application` state machine, panel symbolic icon button, Wayland popup dialog layout, and `Subscription` timer for automatic wallpaper rotation.
- [`src/config.rs`](src/config.rs): `UnsplashSettings` data structures and `SettingsStore` persistence backed by `cosmic-config` (`~/.config/cosmic/io.github.cosmic.AppletUnsplash/v1/settings`).
- [`src/unsplash.rs`](src/unsplash.rs): Async Unsplash API client (`build_random_photo_url`, `select_download_url`, `fetch_and_apply_wallpaper`), `download_location` event reporting, and image downloading.
- [`src/wallpaper.rs`](src/wallpaper.rs): Atomic RON config writer for `~/.config/cosmic/com.system76.CosmicBackground/v1/all` (`set_cosmic_wallpaper`) and cache pruning (`prune_old_wallpapers`).
- [`build.rs`](build.rs) & [`pkgconfig/`](pkgconfig/): Ensures `libxkbcommon.so` and `xkbcommon.pc` resolve cleanly even on hosts where only the runtime `libxkbcommon.so.0` package is installed.
- [`flatpak/io.github.cosmic.AppletUnsplash.json`](flatpak/io.github.cosmic.AppletUnsplash.json): Flatpak manifest built on `org.freedesktop.Platform//25.08`, `org.freedesktop.Sdk//25.08`, and `org.freedesktop.Sdk.Extension.rust-stable//25.08`.

---

## 2. Installed Workspace Skills (`.agents/skills/`)

Activate and follow these workspace skills when performing related maintenance tasks:

| Skill | Path | When to Use |
| :--- | :--- | :--- |
| **`jj`** | [`.agents/skills/jj/SKILL.md`](.agents/skills/jj/SKILL.md) | **Version Control**: This repository is a colocated Jujutsu (`.jj` + `.git`) repository. Always use non-interactive `jj` commands (`jj status`, `jj diff`, `jj describe -m "..."`, `jj commit -m "..."`) instead of mutating `git` commands. |
| **`rust-best-practices`** | [`.agents/skills/rust-best-practices/SKILL.md`](.agents/skills/rust-best-practices/SKILL.md) | **Rust Code Quality**: Follow when writing or refactoring Rust code, error handling, borrowing, and idiomatic API design. |
| **`rust-async-patterns`** | [`.agents/skills/rust-async-patterns/SKILL.md`](.agents/skills/rust-async-patterns/SKILL.md) | **Async / Tokio / Networking**: Follow when modifying `src/unsplash.rs` or `cosmic::task::future` background tasks to keep UI rendering non-blocking. |
| **`rust-testing`** | [`.agents/skills/rust-testing/SKILL.md`](.agents/skills/rust-testing/SKILL.md) | **Unit & Integration Testing**: Follow when adding tests for URL construction, RON serialization, or config validation. |

---

## 3. Core Maintenance & Engineering Rules

### Version Control (`jj`)
- Always run `jj status` and `jj diff` to inspect working-copy changes.
- Use non-interactive flags (`jj describe -m "..."`, `jj commit -m "..."`, `jj new`) — never invoke commands that open an interactive text editor.

### Zero-Warning Policy & Developer Tooling
- Every change **must** pass formatting, lints, and tests with zero warnings across all targets (`make check` or `just check`):
  ```bash
  cargo fmt --all -- --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  cargo build --release
  ```
- Respect [`rustfmt.toml`](rustfmt.toml), [`clippy.toml`](clippy.toml), [`deny.toml`](deny.toml), and the `[lints]` table in [`Cargo.toml`](Cargo.toml) (`unsafe_code = "forbid"`, `unwrap_used = "warn"`).
- Pre-commit hooks live in [`.githooks/pre-commit`](.githooks/pre-commit) (enabled via `make setup-hooks`) and [`.pre-commit-config.yaml`](.pre-commit-config.yaml).
- Keep the `[patch.crates-io]` override for `proc-macro-error2` in [`Cargo.toml`](Cargo.toml) (`patches/proc-macro-error2`) until upstream `libcosmic` / `i18n-embed-fl` updates `proc-macro-error2` past `v2.0.1`.
- Do **not** pin a `rev = ...` on `[dependencies.libcosmic]` in `Cargo.toml`; `cosmic-panel` depends on `libcosmic` without a `rev`, so `Cargo.lock` alone must pin the exact commit to avoid duplicate `libcosmic` graph nodes during Flatpak cargo vendoring.

### UI Performance & Responsiveness
- **Always install `--release` binaries**: `libcosmic` applets use `iced_tiny_skia` (CPU rendering) and `cosmic-text` (font shaping). Debug builds (`opt-level = 0`) cause severe scrolling and typing lag. Always use `make install` (which builds `--release`) when installing to `~/.local/bin/cosmic-applet-unsplash`.
- **No disk I/O on keystrokes**: Never call `self.persist_quietly()` or `store.save()` inside `TextInput::on_input` handlers (`Message::AccessKeyChanged`, `Message::QueryChanged`, `Message::TopicsChanged`, `Message::CollectionsChanged`, `Message::UsernameChanged`). Only persist settings on button clicks (`SaveSettings`, `FetchNow`, enum selectors), popup close (`PopupClosed`, `TogglePopup`), or CLI `--save`.

### Wayland Popup Layout Invariants (`src/window.rs`)
- Pass `size: None` to `self.core.applet.get_popup_settings(...)` so `popup_container` manages sizing.
- Keep `status_col` (`Length::Shrink`) and `form_body` (`scrollable` with `Length::Fill`) inside the fixed-height `middle_section` (`Length::Fixed(420.0)` in popup mode, `Length::Fill` in standalone window mode). This guarantees that when multi-line error or status text is added dynamically while the popup is open, `form_body` shrinks by that exact height and the bottom footer buttons are never pushed off the popup surface.
- Maintain internal padding (`.padding([space_s, space_m, space_s, space_s])`) on the `Column` inside `scrollable` so text inputs and option buttons do not collide with the scrollbar or viewport edges.

### Flatpak & `cosmic-bg` Integration (`src/config.rs`, `src/wallpaper.rs`)
- Always resolve `~/.config` via `config::host_config_dir()` (which inspects `FLATPAK_ID`, `HOST_XDG_CONFIG_HOME`, and `HOME`) rather than raw `dirs::config_dir()`. Inside Flatpak, `dirs::config_dir()` points to the sandbox-private `~/.var/app/.../config`, whereas `cosmic-bg` runs on the host and reads `~/.config/cosmic/com.system76.CosmicBackground/`.
- Store downloaded wallpapers in `~/.config/cosmic/com.system76.CosmicBackground/unsplash/unsplash-<id>-<timestamp>.jpg` with a unique filename per download and write `~/.config/cosmic/com.system76.CosmicBackground/v1/all` atomically via `tempfile::NamedTempFile::persist` so `cosmic-bg`'s `inotify` watcher always detects a new path and reloads the background immediately.
- Only the Unsplash **Access Key** (`Authorization: Client-ID <ACCESS_KEY>`) is required by `GET /photos/random` and `GET /photos/:id/download`. Do not re-add Application ID or Secret Key inputs to the UI.
