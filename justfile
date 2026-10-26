name := 'cosmic-applet-unsplash'
appid := 'io.github.cosmic.AppletUnsplash'

rootdir := ''
prefix := '/usr'

base-dir := absolute_path(clean(rootdir / prefix))
bin-src := 'target' / 'release' / name
bin-dst := base-dir / 'bin' / name

desktop := appid + '.desktop'
desktop-src := 'resources' / desktop
desktop-dst := base-dir / 'share' / 'applications' / desktop

appdata := appid + '.metainfo.xml'
appdata-src := 'resources' / appdata
appdata-dst := base-dir / 'share' / 'metainfo' / appdata

icons-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'

default: build-release

build-debug *args:
    cargo build {{args}}

build-release *args:
    cargo build --release {{args}}

run *args:
    env RUST_BACKTRACE=full cargo run --release -- {{args}}

run-window:
    env RUST_BACKTRACE=full cargo run --release -- --window

test *args:
    cargo test {{args}}

install:
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0644 {{desktop-src}} {{desktop-dst}}
    install -Dm0644 {{appdata-src}} {{appdata-dst}}
    install -Dm0644 resources/icons/hicolor/scalable/apps/{{appid}}.svg {{icons-dst}}/{{appid}}.svg
    install -Dm0644 resources/icons/hicolor/scalable/apps/{{appid}}-symbolic.svg {{icons-dst}}/{{appid}}-symbolic.svg

uninstall:
    rm -f {{bin-dst}} {{desktop-dst}} {{appdata-dst}}
    rm -f {{icons-dst}}/{{appid}}.svg {{icons-dst}}/{{appid}}-symbolic.svg

vendor:
    mkdir -p .cargo
    cargo vendor | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml

flatpak-build:
    flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo builddir flatpak/{{appid}}.json
    flatpak build-bundle repo {{appid}}.flatpak {{appid}}

flatpak-install:
    flatpak-builder --force-clean --user --install builddir flatpak/{{appid}}.json
