# Component inventory and ownership

The Bitter Browser maintainers own release, compatibility and dependency
decisions for this workspace. Report a functional problem in the project
tracker. Use the [private reporting channel](../SECURITY.md) for security issues.
The [support contract](support.md) defines the release targets.

| Crate | Responsibility and entry points | Decision |
| --- | --- | --- |
| `bitter-core` | Profile startup and locking, SQLite history/bookmarks, settings, sessions and site records | Maintain. Schema changes require migration and recovery tests. |
| `bitter-ui` | Desktop application, address bar, tab/workspace sidebar, native preferences and dialogs, generated history/bookmarks pages, WebKit signals | Maintain. Verify UI changes against the real browser. |
| `bitter-cli` | Installed launcher and argument handling. Delegates to the desktop application's profile and running instance | Maintain the single-profile launcher. Track its invocation contract in CLI-01. |
| `bitter-adblock` | Converts supported filter-list rules for WebKit. Browser owns list download, cache and installation | Maintain the supported rule subset. NET-02 owns lifecycle and offline tests. |
| `bitter-net` | Unused networking scaffold | Defer independent networking. WebKit owns browser networking. Remove the scaffold when NET-01 resolves workspace cleanup. |
| `bitter-devtools` | Unused developer-tools scaffold | Defer a custom inspector. Keep using the optional WebKit Inspector. Remove the unused scaffold in a scoped cleanup. |

## Runtime and dependencies

GTK 4 owns native widgets and file selection. Libadwaita supplies the window,
dialogs and preferences. WebKitGTK owns page rendering, network transport, web
storage and web-process isolation. Its runtime follows the package policy in the
support contract. Meson installs the binaries, desktop registration, GSettings
schema and icons. Flatpak and Arch packaging own the distribution boundary.

The Rust manifests record direct ownership. GTK bindings, tracing, URL parsing,
UUIDs and futures connect the desktop application to its runtime. The storage
crate uses bundled SQLite via rusqlite, TOML and Serde for settings and records,
and OS path, file and locking primitives for the profile. Filter rules use
Serde JSON and are compiled by WebKit. The launcher has no third-party Rust
dependencies. The checked-in lockfile records transitive versions. Follow the
[dependency and release workflow](ci-release.md) for audit and packaging changes.

## Local state and entry points

The native application owns the profile lock, settings, SQLite databases,
session snapshot, zoom, site permissions and download records. WebKit owns
cookies, origin storage and network cache. GTK/GSettings stores window geometry
and the OS or portal owns access to user-selected files. The
[profile inventory](profile.md) lists location and lifecycle for each store.

Desktop launches and the CLI enter the same GTK application. The address bar,
keyboard shortcuts, sidebar, toolbar and command palette dispatch browsing
actions. Preferences control effective runtime policies. History and bookmarks
read the local databases. Web-origin permissions, downloads and uploads pass
through native handlers. Product decisions on extensions, account sync, stored
credentials and private browsing follow the support contract and their roadmap
tasks.
