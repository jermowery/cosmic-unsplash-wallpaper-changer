# COSMIC Unsplash Wallpaper Applet (`cosmic-applet-unsplash`)

> [!WARNING]
> **Disclaimer:** This codebase is 100% vibecoded and the author has no idea what they are doing. Use at your own risk!

A native panel applet for the **System76 COSMIC Desktop Environment** (`libcosmic`) that downloads photos from [Unsplash](https://unsplash.com) and sets them as your COSMIC desktop wallpaper via `cosmic-bg`.

---

## How It Works & Codebase Architecture

`cosmic-applet-unsplash` is written in Rust on top of System76's [`libcosmic`](https://github.com/pop-os/libcosmic) toolkit (`iced` + Wayland layer-shell / `xdg_popup`) and `tokio` + `reqwest` (`rustls`).

### End-to-End Flow

1. **Panel Integration**: `cosmic-panel` discovers `io.github.cosmic.AppletUnsplash.desktop` (`X-CosmicApplet=true`) and launches `cosmic-applet-unsplash` as an embedded Wayland panel button.
2. **Configuration Popup**: Clicking the panel icon toggles a Wayland `xdg_popup` dialog (`src/window.rs`) where you configure your Unsplash API Access Key, search filters, target resolution, wallpaper scaling mode, and auto-refresh timer. Middle-clicking the panel icon triggers an immediate wallpaper refresh without opening the popup.
3. **Settings Persistence**: Settings are persisted in RON format via `cosmic-config` at `~/.config/cosmic/io.github.cosmic.AppletUnsplash/v1/settings` (`src/config.rs`), with automatic host-path resolution when running inside a Flatpak sandbox.
4. **Unsplash API Client**: When a wallpaper update is triggered (manually, via CLI, or by the auto-refresh timer), `src/unsplash.rs` queries `GET https://api.unsplash.com/photos/random` with `Authorization: Client-ID <ACCESS_KEY>` and all configured query parameters (`query`, `topics`, `collections`, `username`, `orientation`, `content_filter`, `featured`). It also fires a best-effort request to the photo's `links.download_location` endpoint in compliance with Unsplash API guidelines, then downloads the image at the selected resolution (`Full`, `4K UHD`, `1440p QHD`, `1080p`, or `Raw`).
5. **Applying the Wallpaper (`cosmic-bg`)**: The downloaded image is saved with a unique timestamped filename (`unsplash-<photo_id>-<timestamp>.jpg`) in `~/.config/cosmic/com.system76.CosmicBackground/unsplash/`. `src/wallpaper.rs` atomically updates `~/.config/cosmic/com.system76.CosmicBackground/v1/all` (and `same-on-all`) with the new `Path(...)` and `ScalingMode`. `cosmic-bg`'s file watcher (`inotify`) immediately detects the change and renders the new wallpaper across your display(s). Older cached images beyond the 10 most recent downloads are automatically pruned.

### Project Structure

```text
.
├── Cargo.toml                  # Crate metadata, dependencies (libcosmic, reqwest, tokio, ron, serde)
├── build.rs                    # Build script ensuring libxkbcommon.so linker discovery
├── Makefile / justfile         # Build, install, vendor, and Flatpak packaging recipes
├── LICENSE                     # GNU General Public License v3.0 (GPL-3.0-only)
├── flatpak/
│   └── io.github.cosmic.AppletUnsplash.json   # Flatpak manifest (Cosmic.BaseApp + freedesktop 25.08)
├── resources/
│   ├── io.github.cosmic.AppletUnsplash.desktop      # COSMIC panel applet desktop entry
│   ├── io.github.cosmic.AppletUnsplash.metainfo.xml # AppStream metadata
│   └── icons/hicolor/scalable/apps/                 # Full-color and symbolic SVG panel icons
└── src/
    ├── main.rs                 # Entry point supporting panel applet mode, --window mode, and CLI --fetch-now
    ├── window.rs               # libcosmic Application implementation, panel icon, popup UI, and timer subscription
    ├── config.rs               # UnsplashSettings data model and cosmic-config RON persistence
    ├── unsplash.rs             # Async Unsplash REST API client, URL builder, and image downloader
    └── wallpaper.rs            # Atomic RON writer for com.system76.CosmicBackground and cache pruning
```

---

## Features

- **COSMIC Panel Integration**: Sits in the COSMIC top/bottom/side panel or dock (`X-CosmicApplet=true`) and opens a native configuration popup dialog when clicked.
- **Middle-Click Quick Refresh**: Middle-click the panel icon at any time to immediately fetch and apply a new random Unsplash wallpaper.
- **Full Unsplash API Configuration**:
  - **API Key**: Input your Unsplash **Access Key** (`Client-ID`) with a visibility toggle.
  - **Search Terms (`query`)**: Filter random photos by search keywords (e.g., `mountains, nature`, `space`, `cyberpunk`).
  - **Topics (`topics`)**: Filter by comma-separated Unsplash topic slugs or IDs (e.g., `wallpapers,nature`).
  - **Collections (`collections`)**: Filter by comma-separated public Unsplash collection IDs.
  - **Photographer (`username`)**: Restrict photos to a specific Unsplash user (`@username`).
  - **Featured Only (`featured`)**: Restrict selection to Unsplash curated/featured photos.
  - **Orientation (`orientation`)**: Choose `Any`, `Landscape` (default), `Portrait`, or `Squarish`.
  - **Content Safety (`content_filter`)**: Choose `Standard (Low)` or `Strict (High)`.
  - **Image Resolution**: Choose `Full`, `4K UHD (3840x2160)`, `1440p QHD (2560x1440)`, `1080p`, or `Raw`.
- **COSMIC Desktop & Rotation Options**:
  - **Wallpaper Scaling Mode**: `Zoom (Fill)`, `Fit`, or `Stretch` (written directly to `~/.config/cosmic/com.system76.CosmicBackground/v1/all`).
  - **Auto-Refresh Timer**: Automatically rotate wallpapers every `15m`, `30m`, `1h`, `6h`, or `24h` (or keep on `Off` for manual updates).
  - **Attribution & Cache Management**: Displays photographer attribution (`Photo by Name (@username)`) in the applet popup, triggers Unsplash's `download_location` tracking endpoint, and automatically prunes older cached wallpapers in `~/.config/cosmic/com.system76.CosmicBackground/unsplash/`.

---

## Getting an Unsplash API Key (Required)

This applet does not bundle a shared API key — **you must provide your own Unsplash API Access Key**. Creating a developer account and registering an application on Unsplash is free:

1. Read the official Unsplash guide on [**Creating a Developer Account & Registering Your Application**](https://unsplash.com/documentation#creating-a-developer-account).
2. Go to the [**Unsplash Developers Applications Portal**](https://unsplash.com/oauth/applications) and click **New Application**.
3. Accept the API Use and Guidelines terms and give your application a name (e.g., `COSMIC Wallpaper Applet`).
4. Scroll down to the **Keys** section of your application page and copy the **Access Key** *(you do not need the Application ID or Secret Key)*.
5. Paste the **Access Key** into the applet's **Unsplash API Key** field in the panel popup dialog (or pass `--access-key "<YOUR_ACCESS_KEY>"` / set `UNSPLASH_ACCESS_KEY` in your environment).

---

## Prerequisites

- **Rust Toolchain**: `rustc` and `cargo` (Rust 1.80+ recommended)
- **System Libraries** (for native host builds on Debian/Ubuntu/Pop!_OS):
  ```bash
  sudo apt install build-essential pkg-config libwayland-dev libxkbcommon-dev
  ```
- **Flatpak Tools** (optional, for Flatpak packaging):
  ```bash
  sudo apt install flatpak flatpak-builder
  ```

---

## Building the Applet

### Debug Build
```bash
cargo build
```

### Release Build (Recommended for Smooth UI Performance)
```bash
cargo build --release
```
Or using `make` / `just`:
```bash
make release
# or
just build-release
```
The compiled binary will be located at `target/release/cosmic-applet-unsplash`.

### Running Unit Tests
```bash
cargo test
```

---

## Developer Experience & Tooling

This repository includes standard Rust developer tooling out of the box:

- **Formatter (`rustfmt`)**: Configured in [`rustfmt.toml`](rustfmt.toml) (`max_width = 100`, import & module reordering, shorthand idioms).
  ```bash
  make fmt          # or: just fmt / cargo fmt --all
  make fmt-check    # or: just fmt-check / cargo fmt --all -- --check
  ```
- **Linter (`clippy`)**: Configured in [`Cargo.toml`](Cargo.toml) (`[lints.rust]` and `[lints.clippy]`) and [`clippy.toml`](clippy.toml) with `unsafe_code = "forbid"` and zero warnings allowed.
  ```bash
  make lint         # or: just lint / cargo clippy --all-targets -- -D warnings
  ```
- **All-in-One Quality Gate**: Run formatting check, Clippy lints, and unit tests in a single command:
  ```bash
  make check        # or: just check
  ```
- **Pre-Commit Hooks**: Both native Git hooks ([`.githooks/pre-commit`](.githooks/pre-commit)) and [`pre-commit`](https://pre-commit.com/) ([`.pre-commit-config.yaml`](.pre-commit-config.yaml)) are included.
  - To enable the zero-dependency native Git hook (`cargo fmt`, `cargo clippy`, `cargo test`):
    ```bash
    make setup-hooks   # or: just setup-hooks
    ```
  - Or if you use the `pre-commit` framework:
    ```bash
    pre-commit install
    ```
- **Dependency & License Auditing (`cargo-deny`)**: Configured in [`deny.toml`](deny.toml) for security advisories, license compliance, crate bans, and allowed registries.
  ```bash
  make audit        # or: just audit / cargo deny check
  ```
- **Toolchain & Editor Config**:
  - [`rust-toolchain.toml`](rust-toolchain.toml) pins the `stable` channel with `rustfmt`, `clippy`, `rust-src`, and `rust-analyzer` components.
  - [`.editorconfig`](.editorconfig) and [`.vscode/`](.vscode/) configure `rust-analyzer` (with `clippy` on save and format-on-save) and recommended extensions.
  - [`.github/dependabot.yml`](.github/dependabot.yml) keeps Cargo dependencies and GitHub Actions updated weekly.

---

## Running the Applet

### 1. Installing & Running in the COSMIC Panel
To build the optimized release binary and install the applet locally for your user account (`~/.local/bin` and `~/.local/share/applications`):
```bash
make install
```
*(Or system-wide to `/usr` with `sudo just install`.)*

Once installed:
1. Open **COSMIC Settings** → **Desktop** → **Panel** (or **Dock**) → **Configure Panel Applets**.
2. Click **Add Applet** and select **Unsplash Wallpaper** (`io.github.cosmic.AppletUnsplash`).
3. Click the **Unsplash Wallpaper** icon in your panel to open the settings popup dialog, enter your Unsplash **Access Key** and search preferences, and click **New Wallpaper** (or **Save & Change Wallpaper**).

### 2. Running the Settings Dialog in a Standalone Window
You can also launch the configuration UI directly as a standalone desktop window for testing or configuration outside the panel:
```bash
cargo run --release -- --window
```

### 3. CLI / Headless One-Shot Wallpaper Update
You can trigger a wallpaper fetch and update directly from the command line (and optionally save settings to `cosmic-config`):
```bash
./target/release/cosmic-applet-unsplash \
  --access-key "<YOUR_UNSPLASH_ACCESS_KEY>" \
  --query "mountains, landscape" \
  --orientation landscape \
  --content-filter high \
  --save \
  --fetch-now
```

---

## Packaging the Applet for Flatpak

A complete Flatpak manifest is provided at [`flatpak/io.github.cosmic.AppletUnsplash.json`](flatpak/io.github.cosmic.AppletUnsplash.json), built on top of `org.freedesktop.Platform//25.08`, `org.freedesktop.Sdk.Extension.rust-stable//25.08`, and `com.system76.Cosmic.BaseApp//stable`.

### 1. Install Required Flatpak Runtimes & SDKs
```bash
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --user --if-not-exists cosmic https://apt.pop-os.org/cosmic/cosmic.flatpakrepo

flatpak install --user flathub \
  org.freedesktop.Platform//25.08 \
  org.freedesktop.Sdk//25.08 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08

flatpak install --user cosmic com.system76.Cosmic.BaseApp//stable
```

### 2. (Optional) Vendor Cargo Dependencies for Offline Flatpak Builds
For reproducible offline builds (such as Flathub CI or offline `flatpak-builder`), vendor the Rust dependencies first:
```bash
make vendor
# or: just vendor
```
*(Note: The manifest also enables `--share=network` in `build-options` when `.cargo/config.toml` is not present, so local `flatpak-builder` builds work both with and without pre-vendoring.)*

### 3. Build and Install the Flatpak Locally
```bash
flatpak-builder --force-clean --user --install builddir flatpak/io.github.cosmic.AppletUnsplash.json
```
Or via `make` / `just`:
```bash
make flatpak-install
# or
just flatpak-install
```

### 4. Export a Single-File `.flatpak` Bundle
To package the applet into a redistributable `io.github.cosmic.AppletUnsplash.flatpak` bundle:
```bash
flatpak-builder --force-clean --user --repo=repo builddir flatpak/io.github.cosmic.AppletUnsplash.json
flatpak build-bundle repo io.github.cosmic.AppletUnsplash.flatpak io.github.cosmic.AppletUnsplash
```
Or via `make` / `just`:
```bash
make flatpak-build
# or
just flatpak-build
```

To install the resulting `.flatpak` bundle on any COSMIC system:
```bash
flatpak install --user io.github.cosmic.AppletUnsplash.flatpak
```

---

## Configuration Storage Paths

- **Applet Settings**: `~/.config/cosmic/io.github.cosmic.AppletUnsplash/v1/settings`
- **Downloaded Wallpapers**: `~/.config/cosmic/com.system76.CosmicBackground/unsplash/`
- **COSMIC Background Config**: `~/.config/cosmic/com.system76.CosmicBackground/v1/all`

---

## License

This project is licensed under the **GNU General Public License v3.0** (`GPL-3.0-only`). See [LICENSE](LICENSE) for full details.
