# Release and support contract

Policy revision 1, effective from the 0.4 release series.

Bitter Browser is an experimental, local-first desktop browser for Linux users
who want a GTK browser for everyday navigation, downloads, bookmarks and history.
The matrix defines the configurations maintainers target for release testing.
Each release's check record documents the configurations actually exercised.

| Distribution path | Architecture | Display sessions | Runtime policy |
| --- | --- | --- | --- |
| Arch Linux package on up-to-date Arch Linux | x86-64 | Wayland and X11 | Use the distribution's current security-maintained WebKitGTK 6.0 package. |
| Flatpak on desktop Linux with working user portals | x86-64 | Wayland and X11 fallback | Use the GNOME runtime pinned in the Flatpak manifest (51 for 0.4). Keep the app and runtime updated together. |

The build uses the WebKitGTK 6.0 API with WebKitGTK 2.44 as its API floor.
A runtime must also receive security updates from its distributor. Upgrade the
pinned Flatpak runtime before its security support ends. When a supported runtime
has an unpatched issue that affects the browser, document the affected path and
withhold a production release for it until there is a fix or a verified mitigation.
Browser-specific fixes target the latest release, as described in the
[security policy](../SECURITY.md).

Source builds, other distributions using native packages, non-x86-64 systems,
Windows, macOS and mobile are outside this matrix. The product scope excludes
browser extensions, account sync, a password manager and private browsing.
English is the initial browser UI language. Web pages use the languages they
provide.

## Default privacy behavior

A new profile stores settings, bookmarks, history, session state and website data
locally. Session restore, content blocking, tracking prevention and third-party
cookie protection start enabled. HTTPS-only mode starts disabled. Site
permissions require a decision where the browser supports the request, and
remembered decisions can be revoked. There is no product telemetry. Network
activity includes page loads, searches sent to the configured search provider,
and filter-list updates. See [Profiles and local data](profile.md) and
[Security logging](security-logging.md).

## Accessibility and release criteria

Before calling a release production-ready, verify that a keyboard user can reach
and operate address, tab, download, settings and permission controls with visible
focus. Verify meaningful names and state with Orca, and verify usable layout at
200% scaling and in high contrast. Record native Wayland and X11 evidence, with
any unresolved barriers documented as release blockers.

The release owner must record a clean install, upgrade and launch for each
packaging path, verify core browsing and profile recovery, and confirm that the
security, compatibility and quality gates in the [roadmap](../TASKS.md) have
evidence. Automated CI, dependency audit and signed release artifacts are
required. A missing or failing P0 gate blocks the production designation. See
[CI and releases](ci-release.md) for the release procedure.

Use the [production release gates](release-gates.md) to record the candidate
evidence and sign-off.
