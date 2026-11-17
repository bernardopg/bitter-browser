# Bitter Browser

Bitter Browser is an early-stage Linux browser built with Rust, GTK 4, libadwaita and WebKitGTK 6. The application currently provides an experimental window, tabs, navigation and local history. It is not yet a production-ready browser: several workspace crates are placeholders and features described in the original project prompt are not necessarily implemented.

## Install

The [latest release](https://github.com/bernardopg/bitter-browser/releases/latest) provides an x86-64 Flatpak bundle. Install Flathub first, download the bundle and run:

```sh
flatpak install --user flathub org.gnome.Platform//51
flatpak install --user ./bitter-browser-0.1.1-x86_64.flatpak
flatpak run com.bitterbrowser.Browser
```

Check the checksum published alongside the bundle. The version in the filename changes with each release. Flatpak builds require GNOME SDK 51 and the matching Freedesktop Rust SDK extension.

## Develop

Install stable Rust with rustfmt and Clippy, GTK >= 4.14, libadwaita >= 1.6, WebKitGTK 6 >= 2.44, and their development headers. Meson and Ninja are needed for installation.

```sh
cargo build --workspace --locked
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked
cargo run --locked -p bitter-ui
```

The application needs a graphical session. For packaging use `meson setup builddir`, `meson compile -C builddir`, and `meson install -C builddir`. See [contributing](CONTRIBUTING.md), [architecture](docs/architecture.md), and [CI and release](docs/ci-release.md).

## Project

Report bugs and request features through [issues](https://github.com/bernardopg/bitter-browser/issues). For security reports follow [SECURITY.md](SECURITY.md). Contributions are subject to the [code of conduct](CODE_OF_CONDUCT.md). Licensed under [MPL-2.0](LICENSE).
