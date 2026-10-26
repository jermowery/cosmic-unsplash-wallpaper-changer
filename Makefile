NAME := cosmic-applet-unsplash
APPID := io.github.cosmic.AppletUnsplash
PREFIX ?= $(HOME)/.local

.PHONY: all build release run run-window test install uninstall vendor flatpak-build flatpak-install clean

all: release

build:
	cargo build

release:
	cargo build --release

run:
	cargo run --release

run-window:
	cargo run --release -- --window

test:
	cargo test

install: release
	install -Dm0755 target/release/$(NAME) $(PREFIX)/bin/$(NAME)
	sed "s|Exec=$(NAME)|Exec=$(PREFIX)/bin/$(NAME)|g" resources/$(APPID).desktop > /tmp/$(APPID).desktop
	install -Dm0644 /tmp/$(APPID).desktop $(PREFIX)/share/applications/$(APPID).desktop
	rm -f /tmp/$(APPID).desktop
	install -Dm0644 resources/$(APPID).metainfo.xml $(PREFIX)/share/metainfo/$(APPID).metainfo.xml
	install -Dm0644 resources/icons/hicolor/scalable/apps/$(APPID).svg $(PREFIX)/share/icons/hicolor/scalable/apps/$(APPID).svg
	install -Dm0644 resources/icons/hicolor/scalable/apps/$(APPID)-symbolic.svg $(PREFIX)/share/icons/hicolor/scalable/apps/$(APPID)-symbolic.svg

uninstall:
	rm -f $(PREFIX)/bin/$(NAME)
	rm -f $(PREFIX)/share/applications/$(APPID).desktop
	rm -f $(PREFIX)/share/metainfo/$(APPID).metainfo.xml
	rm -f $(PREFIX)/share/icons/hicolor/scalable/apps/$(APPID).svg
	rm -f $(PREFIX)/share/icons/hicolor/scalable/apps/$(APPID)-symbolic.svg

vendor:
	mkdir -p .cargo
	cargo vendor | head -n -1 > .cargo/config.toml
	echo 'directory = "vendor"' >> .cargo/config.toml

flatpak-build:
	flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo builddir flatpak/$(APPID).json
	flatpak build-bundle repo $(APPID).flatpak $(APPID)

flatpak-install:
	flatpak-builder --force-clean --user --install builddir flatpak/$(APPID).json

clean:
	cargo clean
	rm -rf builddir repo .flatpak-builder $(APPID).flatpak
