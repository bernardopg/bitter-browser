# Browser threat model

Scope revision 1 for the 0.4 release series. The target is a desktop Linux user
browsing with an ordinary, unprivileged account on the Arch or Flatpak path.
Assets include the user's local files, browsing history, bookmarks, session
state, cookies, site permissions and control of device access. The browser has
one persistent WebKit network session shared by its workspaces. Workspaces
organize tabs and share the same site data and permission decisions.

| Boundary | Threat and assets at risk | Current controls and verification needed |
| --- | --- | --- |
| Remote page or popup → web process → native UI | A malicious page tries to reach another origin's data, local files or native commands. | WebKit enforces web origins and supplies the web-process sandbox. Native navigation and popup policy govern dispatch and related tabs. Verify isolation and the effective package permissions on both display backends (SEC-01, SEC-02). |
| Saved web titles and URLs → generated internal pages → native actions | A page supplies text that later renders in history or bookmarks, or attempts to invoke a native history action. | Generated pages escape saved text and attributes. The history deletion handler checks the internal history source. Validate internal page isolation and the action boundary in the actual browser (SEC-03). |
| Remote network → navigation and subresources | A network intermediary serves an untrusted certificate or attempts a secure-to-insecure downgrade. | WebKit validates certificates and the browser provides a blocking error view. Optional HTTPS-only mode upgrades HTTP. Verify valid and invalid chains, downgrade behavior and mixed content (SEC-04). |
| Web origin → device and notification requests | A site tries to capture location, camera, mic or screen data, or inherit another origin's consent. | The browser mediates supported requests, remembers supported decisions by origin and never remembers screen-sharing consent. Verify denial, revocation and origin separation (SEC-05). |
| Website → download destination or upload chooser | An untrusted filename collides with local files, or a page tries to select a file the user did not authorize. | The browser sanitizes proposed download names and avoids overwriting existing files. GTK handles file selection. Verify the Flatpak portal path and its grants (UI-03). |
| Remote filter list → local conversion and WebKit compilation | Broken or oversized list content disables blocking or consumes resources. | The browser caps downloaded files before conversion and keeps compiled filters. Verify network rules and site exceptions with controlled fixtures (NET-02). |
| Local process or interrupted write → profile | A separate OS account reads private browsing data, or an interrupted operation corrupts it. | Owner-only profile directories, profile locking, SQLite migrations and atomic settings/session writes address these risks. Inspect filesystem and recovery guarantees in [Profiles and local data](profile.md). |

The trusted computing base includes the OS, GTK, WebKitGTK, the package runtime,
the native browser process and the profile's OS account. A compromised OS,
an attacker with root or access to that account, and an unpatched engine escape
can defeat these boundaries. Profile ownership controls access by other OS
users. The profile provides no encryption against account compromise. The
Flatpak app sandbox and WebKit process sandbox are separate layers. Flatpak's
manifest grants network access and the download directory, among other runtime
permissions. Native Arch builds run with the desktop account's permissions.
Review effective access during SEC-01.

Private browsing, browser extensions, a password manager and account sync are
outside scope. The browser provides no product telemetry. Diagnostic handling
and reporting are described in [Security logging](security-logging.md) and the
[security policy](../SECURITY.md).

## Review and acceptance

An independent reviewer must check this model against the navigation, profile,
WebKit and packaging implementations, including the assumptions about internal
page origin, sandbox and portal access. Record findings and dispositions under
PRD-02 and the linked security tasks in the [roadmap](../TASKS.md). Runtime
verification and review remain production release gates.
