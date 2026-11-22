# Architecture and current scope

The Cargo workspace contains `bitter-ui` (GTK/libadwaita window and WebKit integration), `bitter-core` (configuration, paths, bookmarks and history), `bitter-cli` (command-line launcher that forwards addresses to `bitter-ui`), `bitter-adblock` (content filter compilation), and two scaffold crates: `bitter-net` and `bitter-devtools`. The scaffold crates are not linked by the UI yet. Inspect the source and tests before relying on a feature. `meson.build` installs the UI, CLI, desktop metadata, schema and icon; `flatpak/` and `PKGBUILD` describe distribution builds.

Web content runs through WebKitGTK and the permissions of the installed application. The project has not established a security audit, cross-platform support, or compatibility with all browser features. Architecture and APIs may change while the project is experimental.

Startup opens and locks the [local profile](profile.md) before initializing
history, bookmarks or WebKit. Tabs share one persistent WebKit network session.
Configuration and SQLite failures report through the native UI. The GTK
preferences dialog edits the values the browser applies.
