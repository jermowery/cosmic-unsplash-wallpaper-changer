name := 'cosmic-ext-applet-random-wallpaper'
appid := 'io.github.jermowery.CosmicExtAppletRandomWallpaper'

rootdir := ''
prefix := '/usr'

appdata := appid + '.metainfo.xml'
desktop := appid + '.desktop'

# Installation paths
base-dir := absolute_path(clean(rootdir / prefix))
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
appdata-src := 'resources' / appdata
appdata-dst := base-dir / 'share' / 'metainfo' / appdata
bin-src := cargo-target-dir / 'release' / name
bin-dst := base-dir / 'bin' / name
desktop-src := 'resources' / desktop
desktop-dst := base-dir / 'share' / 'applications' / desktop
icons-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'
icon-dst := icons-dst / appid + '.svg'
symbolic-icon-dst := icons-dst / appid + '-symbolic.svg'

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
lint *args:
    cargo clippy --all-targets {{args}} -- -D warnings

# Runs a clippy check with JSON message format
check-json: (lint '--message-format=json')

# Formats all Rust code
fmt:
    cargo fmt --all

# Checks Rust formatting
fmt-check:
    cargo fmt --all -- --check

# Runs unit tests
test *args:
    cargo test {{args}}

# Runs formatting, clippy, and unit tests
check: fmt-check lint test

# Runs cargo-deny audit
audit:
    cargo deny check

# Configures git pre-commit hooks
setup-hooks:
    chmod +x .githooks/pre-commit
    git config core.hooksPath .githooks
    @echo "Git pre-commit hooks enabled (.githooks/pre-commit)"

# Run the application for testing purposes
run *args:
    env RUST_BACKTRACE=full cargo run --release -- {{args}}

# Run the standalone settings window for testing purposes
run-window:
    env RUST_BACKTRACE=full cargo run --release -- --window

# Installs files
install:
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0644 {{desktop-src}} {{desktop-dst}}
    install -Dm0644 {{appdata-src}} {{appdata-dst}}
    install -Dm0644 resources/icons/hicolor/scalable/apps/{{appid}}.svg {{icon-dst}}
    install -Dm0644 resources/icons/hicolor/scalable/apps/{{appid}}-symbolic.svg {{symbolic-icon-dst}}

# Uninstalls installed files
uninstall:
    rm -f {{bin-dst}} {{desktop-dst}} {{appdata-dst}} {{icon-dst}} {{symbolic-icon-dst}}

# Vendor dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    echo >> .cargo/config.toml
    tar pcf vendor.tar .cargo vendor
    rm -rf .cargo vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Bump cargo version, create commit, and create tag
tag version:
    find -type f -name Cargo.toml -exec sed -i '0,/^version/s/^version.*/version = "{{version}}"/' '{}' \; -exec git add '{}' \;
    cargo check
    cargo clean
    git add Cargo.lock
    git commit -m 'release: {{version}}'
    git tag -a {{version}} -m ''

# Build Flatpak bundle locally
flatpak-build:
    flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo builddir flatpak/{{appid}}.json
    flatpak build-bundle repo {{appid}}.flatpak {{appid}}

# Install Flatpak locally
flatpak-install:
    flatpak-builder --force-clean --user --install builddir flatpak/{{appid}}.json
