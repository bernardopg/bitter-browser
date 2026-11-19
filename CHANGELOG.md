<!-- markdownlint-disable MD024 -->
# Changelog

All notable changes to Bitter Browser are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [0.2.1] - 2026-11-19

### Fixed

- The Arch `PKGBUILD` attached to the release downloaded GitHub's generated
  archive, whose checksum differs from the published source archive, so
  `makepkg` rejected it. It now downloads the release asset, and the release
  workflow verifies every published asset and the `PKGBUILD` source after
  publishing.

## [0.2.0] - 2026-11-19

### Added

- Downloads to the XDG download directory with safe, unique file names,
  progress, cancel, open and show in folder (Ctrl+J).
- Find in page (Ctrl+F), per-tab zoom (Ctrl+Plus, Ctrl+Minus, Ctrl+0) and full
  screen (F11) for the browser, videos and pages.
- Site icons in the sidebar, including restored tabs before their page loads.
- Window size and maximized state are kept between launches.
- Main and Second workspaces with their own tab order and selection, keyboard
  reordering, reopening closed tabs and background links.
- Session restore after normal exits and crashes, creating WebKit views only
  when a tab is first selected.
- Address bar suggestions from history, bookmarks and open tabs, queried off
  the interface thread.
- Loading progress with Stop and native error pages with retry.
- Native preferences for the search provider, color scheme and startup.
- Links opened by other applications open in the running window.
- `--version` and `--help` command-line options.
- Release pipeline that publishes a Flatpak bundle, a source archive, an Arch
  `PKGBUILD`, an SPDX SBOM, checksums and build provenance attestations.

### Changed

- The Flatpak builds offline from vendored Cargo sources.
- Arch packaging gains a release `PKGBUILD` next to the development one, both
  following the Arch Rust package guidelines.
- Unused dependencies were removed, shrinking the dependency graph from 310 to
  159 crates, and the Flatpak no longer requests the Secret Service.

### Fixed

- The window background and the sidebar text colors in the light scheme.
- A GTK warning when closing the window with the address bar focused.

## [0.1.1] - 2026-11-17

### Changed

- Updated Cargo dependencies, including rustls 0.23.46 and rusqlite 0.43,
  addressing RUSTSEC-2026-0459.
- Updated the Flatpak runtime to GNOME 51.

### Fixed

- Meson installation of the executable and the application icon.

[0.2.1]: https://github.com/bernardopg/bitter-browser/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/bernardopg/bitter-browser/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/bernardopg/bitter-browser/releases/tag/v0.1.1
