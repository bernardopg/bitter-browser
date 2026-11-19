# Production readiness roadmap

This is the tracked roadmap for bringing the current experimental Linux browser to an explicitly defined production release. Unchecked items are work to do, not promises of existing functionality. Use GitHub issues for discussion and assignment; link issues and PRs to the stable task IDs below. A task is complete only when its acceptance evidence is linked from the issue or PR. Split large tasks into smaller issues as needed; do not treat this file as an automatically synchronized issue tracker.

## Current baseline and decisions

- GTK 4/libadwaita and WebKitGTK 6 provide an experimental window, tabs, an omnibar and SQLite-backed history/bookmarks. `crates/bitter-ui/src/window.rs` renders internal HTML pages and opens native preferences through the window action.
- `bitter-adblock` converts Adblock Plus lists (a built-in list, EasyList and EasyPrivacy) into WebKit content blocker rules; the browser downloads, compiles and refreshes them and allows per-site exceptions. HTTPS-only mode, per-origin permissions for notifications, location, camera, microphone and clipboard, `window.open` pop-ups with `window.opener`, the WebKit inspector and clearing browsing data are implemented and covered by the local smoke test. `bitter-net` and `bitter-devtools` are scaffolds; the CLI currently prints a placeholder. These implementations still need the reviews and acceptance evidence that SEC-04 to SEC-07, NET-02 and DEV-01 ask for.
- CI generates workspace-wide coverage, but the last local measurement at the time this roadmap was written was 6.79% of instrumented lines; GUI and real-web behavior were not exercised. Re-measure on the target commit rather than treating this number as permanent.
- Scope is initially Linux x86-64, Wayland/X11 and the documented Flatpak/Arch paths. Product owners must decide minimum supported environments and optional features before promising them. No production-readiness or security audit is implied.

The local implementation of DATA-01, DATA-02, DATA-03 and UI-05 is documented
in [Profiles and local data](docs/profile.md), including verification commands.
The checklist follows the linked-evidence rule above.

The daily browsing tranche covers local session restore (DATA-04),
tab and workspace lifecycle (UI-02), navigation feedback (UI-01), and
asynchronous local address suggestions (UI-01 / TEST-06).
See [Daily browsing](docs/daily-browsing.md) for scope and verification.

The daily tools tranche adds local downloads with safe naming (UI-03, without
upload or portal destination selection yet), find in page, zoom, full screen,
window size persistence and site icons (UI-01 / UI-02), and the `--version`
and `--help` options of the command-line contract (CLI-01).

The packaging tranche makes the Flatpak build offline from vendored crates,
adds a release and a development Arch `PKGBUILD` (OPS-01), and publishes
releases from protected, signed tags with checksums, an SPDX SBOM and build
provenance attestations (OPS-02). See [CI and releases](docs/ci-release.md).
v0.2.1 was installed from its release assets with pacman and Flatpak and used
on a Hyprland Wayland session.

## P0 — Define the product and release gates

- [ ] **PRD-01 — Product contract.** Decide target users, supported Linux distributions/architectures, display servers, WebKitGTK minimum and update policy, accessibility targets, privacy defaults, and explicitly unsupported features. Publish a versioned support matrix and release criteria. Depends: none.
- [ ] **PRD-02 — Threat model.** Map untrusted pages, extensions if supported, internal pages, native processes, profiles and filesystem; document trust boundaries, attackers, security assumptions, abuse cases and residual risk. Have it independently reviewed. Depends: PRD-01.
- [ ] **PRD-03 — Inventory and ownership.** Audit each workspace crate, dependency, runtime, data store and UI entry point; identify owner/maintainer and choose implement, defer or remove for every placeholder. Ensure claims in README and settings match actual capabilities. Depends: PRD-01.
- [ ] **PRD-04 — Quality gates.** Define measurable security, functional, compatibility, accessibility, performance, recovery and support gates; include test environments, evidence, sign-off owner and explicit release blocker policy. No gate may be satisfied solely by a coverage percentage. Depends: PRD-01, PRD-02.

## P0 — Browser security and privacy

- [ ] **SEC-01 — Engine and sandbox review.** Verify WebKitGTK process isolation and sandbox behavior on every supported packaging/display configuration; document actual runtime permissions and test a hostile local page against filesystem/network boundaries. Fix or block configurations that violate PRD-02. Depends: PRD-02.
- [ ] **SEC-02 — Navigation and origin policy.** Specify and enforce scheme allowlists, external protocol handling, new windows/popups, redirects and downloads; regression-test malformed URLs, `file:`, custom schemes and dangerous navigation paths. Depends: PRD-02.
- [ ] **SEC-03 — Internal pages.** Audit `bitter://` and `load_html` origins, data-to-HTML escaping, links, CSP and privilege boundaries; run isolated browser regressions with hostile titles/URLs and show that web pages cannot access privileged resources. Depends: SEC-02.
- [ ] **SEC-04 — TLS and transport.** Verify certificate error handling, mixed content, HTTPS-only behavior and redirect downgrade handling; never silently bypass certificate errors. Test with disposable local CA and invalid certificates. Depends: PRD-02.
- [ ] **SEC-05 — Permissions and device access.** Define explicit per-origin consent, persistence, revocation and safe defaults for location, camera, microphone, clipboard, notifications and related WebKit requests; test denial, expiry and origin changes. Depends: PRD-02.
- [ ] **SEC-06 — Privacy controls.** Implement and test settings actually offered to users, including cookie/storage policy, tracker/ad blocking and HTTPS-only mode; otherwise remove or clearly mark controls as unavailable. Prove blocking behavior against deterministic local fixtures, not just configuration flags. Depends: PRD-03, SEC-02.
- [ ] **SEC-07 — Private profiles and data clearing.** Define private-session isolation and lifecycle, storage/cookie/cache/history clearing and user-visible scope; test persistence, restart and isolation on both supported installation methods. Depends: DATA-01.
- [ ] **SEC-08 — Secrets, telemetry and logging.** Define data collection policy; avoid collecting credentials or browsing history in logs/crash reports; protect any stored secrets using an appropriate OS secret service, with denial/unavailable behavior tested. No production telemetry without an explicit user-facing policy. Depends: PRD-02.
- [ ] **SEC-09 — Security verification and response.** Establish supported-version, vulnerability disclosure, triage, fix and coordinated-release procedures; perform dependency audit, focused independent security review and threat-model regression before release. Track findings through closure. Depends: SEC-01–08.

## P0 — Data safety and reliability

- [ ] **DATA-01 — Profile model.** Define profile paths, permissions, locking, XDG/Flatpak behavior and multi-instance semantics; replace silent path-creation failures and literal `~` fallbacks with explicit, tested errors. Verify isolated temporary profiles and concurrent launches. Depends: PRD-01.
- [ ] **DATA-02 — Storage lifecycle.** Add versioned SQLite schema migrations and rollback/recovery strategy for history/bookmarks; test empty, old, corrupted, read-only and disk-full databases without touching real user profiles. Depends: DATA-01.
- [ ] **DATA-03 — Configuration integrity.** Validate and migrate configuration; handle malformed/partial/unknown data, atomic saves and failed writes visibly without silently destroying user settings. Test interrupted writes and restarts. Depends: DATA-01.
- [ ] **DATA-04 — Session restore.** Specify tab/workspace state and private-session exclusions, then implement atomic persistence, clean shutdown and crash recovery; test corrupted session files, concurrent changes and restart. Depends: DATA-01, UI-02.
- [ ] **DATA-05 — Backup and import/export.** Define portable bookmark/history backup format and conflict policy; implement validation, safe recovery and user confirmation; test malicious and oversized input. Depends: DATA-02.
- [ ] **REL-01 — Failure handling.** Replace silent degraded states with actionable UI, preserve data on failure, handle web-process crashes and restart/recovery, and test offline, process termination and storage failures. Depends: DATA-01–03.

## P0 — Core browsing behavior

- [ ] **UI-01 — Navigation correctness.** Test address/search interpretation, back/forward/reload, redirects, page title/URL updates, loading errors and HTTP error pages; implement predictable focus, state and cancellation behavior. Depends: SEC-02.
- [ ] **UI-02 — Tabs, windows and workspaces.** Specify ownership and lifecycle; implement and test close-last-tab, reorder, background tabs, new window, popup, workspace switch and keyboard behavior without losing state. Depends: PRD-01.
- [ ] **UI-03 — Downloads and uploads.** Implement explicit destination selection, overwrite and cancellation handling, progress, failure reporting and safe file-opening behavior; test Flatpak portal paths and untrusted filenames. Depends: SEC-02, DATA-01.
- [ ] **UI-04 — Bookmarks and history.** Complete CRUD, search, delete/clear, escaping and error states; prove persistence and timestamp/FTS behavior on real temporary SQLite databases. Depends: DATA-02, SEC-03.
- [ ] **UI-05 — Settings and browser identity.** Replace the placeholder settings page with accessible controls tied to effective persisted behavior; show security state and errors accurately, including unavailable controls. Depends: DATA-03, SEC-06.
- [ ] **UI-06 — Accessibility and localization.** Meet agreed keyboard-only, screen-reader, focus, high-contrast and scaling requirements; externalize strings and test at least the supported locales. Record manual and automated evidence. Depends: PRD-01.
- [ ] **UI-07 — Web compatibility.** Establish representative local fixtures and a maintained real-site compatibility matrix, including forms, media, downloads, authentication flows and common web APIs. Test privacy/security controls and document unsupported APIs without collecting real credentials. Depends: PRD-04.

## P1 — Networking and optional product capabilities

- [ ] **NET-01 — Network ownership.** Decide whether WebKit or `bitter-net` owns proxy, DNS, TLS and cookie behavior; implement only policies actually enforceable by the engine, document exceptions and test proxy/offline behavior. Depends: PRD-03, SEC-04.
- [ ] **NET-02 — Content blocking lifecycle.** If included in v1, integrate `bitter-adblock` with requests, define filter provenance, refresh, integrity, failure and resource limits; test rules and allowlists. Otherwise defer transparently and remove false UI claims. Depends: SEC-06.
- [ ] **DEV-01 — Developer tools.** Decide whether to expose WebKit inspector or implement additional tools; gate access appropriately and test opening, closing and production defaults. Remove placeholder claims. Depends: PRD-03.
- [ ] **CLI-01 — Command-line contract.** Either implement documented URI, profile, help, version and error semantics or remove the placeholder from shipped packages. Test invalid arguments and running-instance handling. Depends: DATA-01, PRD-03.
- [ ] **EXT-01 — Extensions and credentials decision.** Explicitly decide support for extensions, password management, sync and account features; exclude them from v1 unless threat model, privacy design, consent and maintenance capacity are approved. Depends: PRD-01, PRD-02.

## P0 — Verification and observability

- [ ] **TEST-01 — Unit and integration suite.** Add behavior-driven tests for URL handling, settings, data paths, storage and failure paths; isolate HOME/XDG and use temporary databases. Add regression tests for every confirmed bug. Depends: DATA-01–03, UI-01.
- [ ] **TEST-02 — GUI automation.** Establish reproducible isolated Wayland/X11 or virtual-display tests for launch, tabs, keyboard, internal pages, permissions and crash recovery using local fixtures. Do not use a real browsing profile. Depends: UI-01–06.
- [ ] **TEST-03 — Security integration.** Automate hostile local origins, redirects, certificate failures, permission requests and crafted saved content; record actual browser/engine versions, results and limits. Depends: SEC-01–06.
- [ ] **TEST-04 — Coverage policy.** Measure per-crate and workspace line/branch coverage with documented exclusions; establish evidence-based ratcheting targets after a baseline and enforce no unjustified regressions. Coverage cannot substitute for TEST-02/03. Depends: TEST-01.
- [ ] **TEST-05 — Continuous integration.** Run formatting, lint, unit/integration, coverage, workflow validation, CodeQL and dependency checks on protected PRs; test artifacts and caching safely for untrusted forks. Document required check ownership and failure response. Depends: TEST-01–04.
- [ ] **TEST-06 — Performance and soak.** Define hardware and repeatable startup, navigation, memory, tab-count and long-running benchmarks; investigate regressions and publish reproducible measurements. Depends: PRD-04.

## P0 — Distribution and operations

- [ ] **OPS-01 — Reproducible packaging.** Build and install Flatpak and Arch packages in clean environments; verify runtime/SDK provenance, locked dependencies, icons, desktop registration, portals, upgrade and uninstall. Remove build-time network assumptions or document and mitigate them. Depends: PRD-01, DATA-01.
- [ ] **OPS-02 — Release pipeline and provenance.** Automate version consistency, clean build, SBOM, checksums and provenance/attestation; publish artifacts only from a protected reviewed tag with minimal permissions. Verify downloads and rollback procedure. Depends: OPS-01, TEST-05.
- [ ] **OPS-03 — Updates and vulnerability monitoring.** Define supported WebKit/OS/Flatpak runtime update cadence and response SLA; track sources not covered by Dependabot, validate compatibility and exercise emergency security releases. Depends: PRD-01, SEC-09.
- [ ] **OPS-04 — Diagnostics and incident response.** Document privacy-preserving diagnostics, reproducible bug reports, crash handling, triage, maintainer ownership and security incident escalation; test recovery and release rollback. Depends: SEC-08, REL-01.
- [ ] **OPS-05 — Community and support.** Keep installation, known limitations, compatibility, accessibility, security and contribution docs accurate; define maintenance capacity, issue triage and support expectations. Test published links and package instructions. Depends: PRD-01.

## Release sequence

1. Complete PRD-01–04 and establish owners, scope and threat model. Create linked issues and identify dependencies before implementation.
2. Complete P0 security, data and browsing work alongside regression tests; no privacy or security control is advertised before it is effective and verified.
3. Finish GUI/security integration, packaging and independent security review. Record accepted exceptions with an owner and expiry; any unmet P0 gate blocks production.
4. Run a documented beta on supported configurations, resolve release blockers, then approve and publish a signed/attested production release only when PRD-04 gates are met. Keep rollback and security updates operational afterward.
