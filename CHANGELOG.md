<!-- markdownlint-disable MD024 -->
# Changelog

All notable changes to Bitter Browser are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.4.0] - 2026-11-22

### Added

- Remove a single page from history with the × button on its row of the
  History page (`bitter://history`). Only the History page itself can
  trigger the removal; links on web pages cannot.
- `bitter-cli`, a launcher installed next to `bitter-ui` by Meson, the
  Flatpak and the Arch packages. It accepts `-h`/`--help` and
  `-v`/`--version`, rejects unknown options with exit code 2 and passes
  addresses (and everything after `--`) to `bitter-ui`, returning its exit
  code. `BITTER_BROWSER_BIN` overrides the browser it runs.
- `AGENTS.md` and `CLAUDE.md` with the checks to run before pushing and the
  rule that merged branches are deleted on GitHub and locally.

### Security

- Updated `url` to 2.5.9 for RUSTSEC-2026-0592 (stack overflow in
  `Url::origin()` on nested `blob:` URLs). Bitter only calls `origin()` on
  `http` and `https` addresses, so the advisory was not reachable.
- Downloaded filter lists larger than 8 MiB are rejected before they are
  read into memory.

### Fixed

- Addresses with embedded credentials (`user:pass@`) are no longer recorded in
  history.
- Cycling tabs and reordering tabs no longer panic when a workspace has no
  tabs.

### Changed

- An hourly maintenance workflow deletes the branches of merged pull
  requests and starts CI and CodeQL on `main` after automatic merges, which
  start no workflows by themselves.
- Fixed intermittent CI failures: WebKit unit tests no longer share the GLib
  main context across test threads, and the smoke test waits for history
  writes and retries window focus.
- Expanded the test suite from 58 to 158 tests; line coverage is 46.77%
  (`cargo llvm-cov`), up from 32.7% in 0.3.0.

## [0.3.0] - 2026-11-19

### Added

- Pages opened with `window.open` become the next tab and keep
  `window.opener` and `postMessage`, so sign-in and payment pop-ups work;
  `window.close()` returns to the page that opened them.
- Site permissions: notifications, location, camera, microphone and
  clipboard are asked in a bar above the page and remembered per site;
  screen sharing is asked every time. Remembered answers are reported
  through `navigator.permissions` and managed from the address bar and
  Preferences.
- Ad and tracker blocking with a built-in list, EasyList and EasyPrivacy,
  converted to WebKit content blockers, refreshed every four days, with a
  per-site switch.
- HTTPS-only mode, with a warning page for sites that have no secure version.
- Cross-site tracking prevention and third-party cookie settings, and
  clearing history, cookies and site data, cache and the download list by
  time range (Ctrl+Shift+Delete).
- Context menu items that open links, images and media in a new or
  background tab, and search for the selected text. Hovered links show
  their address.
- Web Inspector (F12, Ctrl+Shift+I), printing (Ctrl+P) and spell checking in
  the system languages.
- A site panel behind the address bar's connection icon.
- Choosing where each download is saved; the download list and per-site
  zoom are kept between launches, with a default zoom setting.
- Redesigned New Tab, History and Bookmarks pages with frequent sites and
  filters, a security indicator, loading progress and a collapsible sidebar
  (F9).

### Changed

- Preferences are split into General, Privacy, Permissions and Advanced
  pages, with search. The placeholder privacy rows are gone.
- WebRTC is enabled. The Flatpak can play sound and use the microphone, and
  the Arch package lists GStreamer plugins for video and hardware decoding
  as optional dependencies.

### Fixed

- Enter in the command palette search field runs the highlighted command.
- Error pages no longer show the secure connection icon.

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

[0.4.0]: https://github.com/bernardopg/bitter-browser/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/bernardopg/bitter-browser/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/bernardopg/bitter-browser/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/bernardopg/bitter-browser/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/bernardopg/bitter-browser/releases/tag/v0.1.1
