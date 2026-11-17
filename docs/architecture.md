# Architecture and current scope

The Cargo workspace contains `bitter-ui` (GTK/libadwaita window and WebKit integration), `bitter-core` (configuration, paths, bookmarks and history), and `bitter-cli`, `bitter-net`, `bitter-adblock`, and `bitter-devtools`. Some of the latter are scaffolds rather than completed implementations. Inspect the source and tests before relying on a feature. `meson.build` installs the UI, desktop metadata, schema and icon; `flatpak/` and `PKGBUILD` describe distribution builds.

Web content runs through WebKitGTK and the permissions of the installed application. The project has not established a security audit, cross-platform support, or compatibility with all browser features. Architecture and APIs may change while the project is experimental.
