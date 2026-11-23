# Production readiness roadmap

This is the tracked roadmap for bringing the current experimental Linux browser to an explicitly defined production release. Unchecked items are work to do, not promises of existing functionality; their status lines describe what already exists. Use GitHub issues for discussion and assignment; link issues and PRs to the stable task IDs below. A task is complete only when its acceptance evidence is linked from the issue or PR. Split large tasks into smaller issues as needed; do not treat this file as an automatically synchronized issue tracker.

## Current state (v0.4.0, 2026-11-22)

Status lines under each task say what exists today and what is still missing.
`[x]` marks tasks whose acceptance work is implemented and tested, with the
evidence in the linked pull requests. Everything else stays open, even when
most of it works, until its remaining gaps are closed.

| Area | State |
| --- | --- |
| Browsing | Tabs in two workspaces, session restore, address suggestions, find, zoom per site, full screen, downloads and uploads, pop-ups as tabs with `window.opener`, context menu, Web Inspector, printing. |
| Privacy and security | Per-site permissions, ad and tracker blocking (built-in list, EasyList, EasyPrivacy), HTTPS-only mode, tracking prevention, third-party cookie policy, clearing browsing data. No private browsing, threat model or security review yet. |
| Data | Locked XDG profile, versioned SQLite migrations, validated atomic configuration, atomic session, site settings, zoom and download records. |
| Packaging | Offline Flatpak, release and `-git` Arch packages, signed tags, SBOM, checksums and provenance attestations. v0.2.1 was installed from its release assets with pacman and Flatpak and used on a Hyprland Wayland session. |
| Verification | 157 automated unit and integration tests, a display test for preferences and an Xvfb smoke test in CI that drives the real browser against loopback fixtures. Line coverage of the automated tests is 46.77% (`cargo llvm-cov`, 2026-11-22); the smoke test is not instrumented. |
| Not started | Product contract, threat model, quality gates (PRD-01 to PRD-04), accessibility and localization, compatibility matrix, private profiles, backup and import, extensions decision. |

Scope is Linux x86-64, Wayland and X11, and the documented Flatpak and Arch
paths. No production readiness or security audit is implied.

Details: [Profiles and local data](docs/profile.md),
[Daily browsing](docs/daily-browsing.md), [CI and releases](docs/ci-release.md).

### Recommended next work

1. PRD-01 and PRD-02: without a product contract and threat model, none of
   the P0 gates can be signed off.
2. TEST-03 with SEC-04 and SEC-05: a disposable local CA, invalid
   certificates, mixed content, and permission denial and origin changes in
   the smoke test.
3. NET-02 gaps: automated tests of network blocking and of the per-site
   exception. A download size limit for filter lists was added; keep the
   remaining coverage behavior-driven.
4. UI-06: keyboard and screen reader review of the new bars, panels and
   pages, and externalized strings.
5. UI-07: a compatibility matrix with sign-in pop-ups, video calls and media
   sites, now that the platform features exist.

## P0 — Define the product and release gates

- [x] **PRD-01 — Product contract.** Decide target users, supported Linux distributions/architectures, display servers, WebKitGTK minimum and update policy, accessibility targets, privacy defaults, and explicitly unsupported features. Publish a versioned support matrix and release criteria. Depends: none.
  *Status:* Policy revision 1 is published in [Release and support contract](docs/support.md).
- [ ] **PRD-02 — Threat model.** Map untrusted pages, extensions if supported, internal pages, native processes, profiles and filesystem; document trust boundaries, attackers, security assumptions, abuse cases and residual risk. Have it independently reviewed. Depends: PRD-01.
  *Status:* [Scope revision 1](docs/threat-model.md) records boundaries, threats and verification. Independent review remains open.
- [x] **PRD-03 — Inventory and ownership.** Audit each workspace crate, dependency, runtime, data store and UI entry point; identify owner/maintainer and choose implement, defer or remove for every placeholder. Ensure claims in README and settings match actual capabilities. Depends: PRD-01.
  *Status:* [Component inventory](docs/components.md) records maintainers, runtime and storage owners, entry points and decisions about deferred components.
- [ ] **PRD-04 — Quality gates.** Define measurable security, functional, compatibility, accessibility, performance, recovery and support gates; include test environments, evidence, sign-off owner and explicit release blocker policy. No gate may be satisfied solely by a coverage percentage. Depends: PRD-01, PRD-02.
  *Status:* Not started.

## P0 — Browser security and privacy

- [ ] **SEC-01 — Engine and sandbox review.** Verify WebKitGTK process isolation and sandbox behavior on every supported packaging/display configuration; document actual runtime permissions and test a hostile local page against filesystem/network boundaries. Fix or block configurations that violate PRD-02. Depends: PRD-02.
  *Status:* Not started. WebKit runs its default web process sandbox; no review per packaging and display configuration.
- [ ] **SEC-02 — Navigation and origin policy.** Specify and enforce scheme allowlists, external protocol handling, new windows/popups, redirects and downloads; regression-test malformed URLs, `file:`, custom schemes and dangerous navigation paths. Depends: PRD-02.
  *Status:* Partial. Typed and external addresses accept only `http`, `https` and `file` (#8); pages cannot open windows without a user gesture and script windows open as related tabs (#15); downloads use safe names (#10). No written policy or malformed-URL regression suite.
- [ ] **SEC-03 — Internal pages.** Audit `bitter://` and `load_html` origins, data-to-HTML escaping, links, CSP and privilege boundaries; run isolated browser regressions with hostile titles/URLs and show that web pages cannot access privileged resources. Depends: SEC-02.
  *Status:* Partial. Internal pages escape titles and addresses, with unit tests (#8, #14). No CSP or isolated hostile-page regression.
- [ ] **SEC-04 — TLS and transport.** Verify certificate error handling, mixed content, HTTPS-only behavior and redirect downgrade handling; never silently bypass certificate errors. Test with disposable local CA and invalid certificates. Depends: PRD-02.
  *Status:* Partial. Certificate failures show a blocking page with no bypass (#8). HTTPS-only mode upgrades pages and handles redirect downgrades, unit-tested and checked against real sites (#15). Missing: tests with a local CA and invalid certificates, mixed content verification.
- [ ] **SEC-05 — Permissions and device access.** Define explicit per-origin consent, persistence, revocation and safe defaults for location, camera, microphone, clipboard, notifications and related WebKit requests; test denial, expiry and origin changes. Depends: PRD-02.
  *Status:* Partial. Per-origin consent bar, persistence, revocation from the address bar and preferences, device lists only for allowed sites, screen sharing never remembered (#15); the smoke test grants and remembers notifications. Missing: tests of denial and origin changes, camera and microphone on real devices, an expiry policy.
- [ ] **SEC-06 — Privacy controls.** Implement and test settings actually offered to users, including cookie/storage policy, tracker/ad blocking and HTTPS-only mode; otherwise remove or clearly mark controls as unavailable. Prove blocking behavior against deterministic local fixtures, not just configuration flags. Depends: PRD-03, SEC-02.
  *Status:* Partial. Content blocking, HTTPS-only, tracking prevention and third-party cookie settings work; element hiding is proven against a local fixture (#15). Missing: deterministic local tests for network blocking and cookie policy.
- [ ] **SEC-07 — Private profiles and data clearing.** Define private-session isolation and lifecycle, storage/cookie/cache/history clearing and user-visible scope; test persistence, restart and isolation on both supported installation methods. Depends: DATA-01.
  *Status:* Partial. Clearing history, cookies and site data, cache and downloads by time range, with a smoke test for history and cookies (#15). WebKitGTK cannot select cookies by age, so ranged clears remove the cookies of sites visited in the range. Private sessions do not exist.
- [ ] **SEC-08 — Secrets, telemetry and logging.** Define data collection policy; avoid collecting credentials or browsing history in logs/crash reports; protect any stored secrets using an appropriate OS secret service, with denial/unavailable behavior tested. No production telemetry without an explicit user-facing policy. Depends: PRD-02.
  *Status:* Partial. No product telemetry is implemented; `docs/security-logging.md` is a draft policy grounded in current code behavior. Missing: a formal log redaction/audit pass, user-facing policy/consent if telemetry is ever added, and secret-service design/tests for any future stored secrets.
- [ ] **SEC-09 — Security verification and response.** Establish supported-version, vulnerability disclosure, triage, fix and coordinated-release procedures; perform dependency audit, focused independent security review and threat-model regression before release. Track findings through closure. Depends: SEC-01–08.
  *Status:* Partial. Private vulnerability reporting, `cargo audit`, CodeQL and dependency review run in CI (#5). No release triage procedure or independent review.

## P0 — Data safety and reliability

- [x] **DATA-01 — Profile model.** Define profile paths, permissions, locking, XDG/Flatpak behavior and multi-instance semantics; replace silent path-creation failures and literal `~` fallbacks with explicit, tested errors. Verify isolated temporary profiles and concurrent launches. Depends: PRD-01.
  *Status:* Locked owner-only XDG profile, explicit errors and concurrent launch behavior (#8). See docs/profile.md.
- [x] **DATA-02 — Storage lifecycle.** Add versioned SQLite schema migrations and rollback/recovery strategy for history/bookmarks; test empty, old, corrupted, read-only and disk-full databases without touching real user profiles. Depends: DATA-01.
  *Status:* Versioned SQLite migrations with rollback and tests on legacy, corrupted, newer-version, read-only and disk-full databases (#8).
- [x] **DATA-03 — Configuration integrity.** Validate and migrate configuration; handle malformed/partial/unknown data, atomic saves and failed writes visibly without silently destroying user settings. Test interrupted writes and restarts. Depends: DATA-01.
  *Status:* Validated configuration with atomic saves, external edit detection and tested failure paths (#8, #15).
- [x] **DATA-04 — Session restore.** Specify tab/workspace state and private-session exclusions, then implement atomic persistence, clean shutdown and crash recovery; test corrupted session files, concurrent changes and restart. Depends: DATA-01, UI-02.
  *Status:* Atomic session snapshots, crash recovery and lazy restore, with smoke tests (#8). No private sessions exist to exclude.
- [ ] **DATA-05 — Backup and import/export.** Define portable bookmark/history backup format and conflict policy; implement validation, safe recovery and user confirmation; test malicious and oversized input. Depends: DATA-02.
  *Status:* Not started.
- [ ] **REL-01 — Failure handling.** Replace silent degraded states with actionable UI, preserve data on failure, handle web-process crashes and restart/recovery, and test offline, process termination and storage failures. Depends: DATA-01–03.
  *Status:* Partial. Web process crashes, network and certificate failures show recovery pages; storage failures show actionable toasts (#8, #10). No offline or disk-full GUI test.

## P0 — Core browsing behavior

- [ ] **UI-01 — Navigation correctness.** Test address/search interpretation, back/forward/reload, redirects, page title/URL updates, loading errors and HTTP error pages; implement predictable focus, state and cancellation behavior. Depends: SEC-02.
  *Status:* Partial. Address and search interpretation, back, forward, reload, stop, redirects, error pages and focus are implemented and covered by unit and smoke tests (#8, #10). Missing: an explicit specification of focus and cancellation.
- [ ] **UI-02 — Tabs, windows and workspaces.** Specify ownership and lifecycle; implement and test close-last-tab, reorder, background tabs, new window, popup, workspace switch and keyboard behavior without losing state. Depends: PRD-01.
  *Status:* Partial. Workspaces, reorder, background tabs, close-last-tab, reopen and pop-ups as related tabs, with smoke tests (#8, #15). The browser has a single window.
- [ ] **UI-03 — Downloads and uploads.** Implement explicit destination selection, overwrite and cancellation handling, progress, failure reporting and safe file-opening behavior; test Flatpak portal paths and untrusted filenames. Depends: SEC-02, DATA-01.
  *Status:* Partial. Destination choice, unique names, overwrite protection, progress, cancel, failure reporting, a persistent list and uploads (#10, #14). Missing: tests under the Flatpak file chooser portal.
- [ ] **UI-04 — Bookmarks and history.** Complete CRUD, search, delete/clear, escaping and error states; prove persistence and timestamp/FTS behavior on real temporary SQLite databases. Depends: DATA-02, SEC-03.
  *Status:* Partial. History and bookmarks with search and filters, clearing history, and deleting single history entries from the history page; only the history page itself can trigger deletion (#8, #14, #15, #30). Missing: editing bookmarks.
- [x] **UI-05 — Settings and browser identity.** Replace the placeholder settings page with accessible controls tied to effective persisted behavior; show security state and errors accurately, including unavailable controls. Depends: DATA-03, SEC-06.
  *Status:* Preferences in four pages tied to effective behavior, with save errors, a site panel showing connection state, and a display test (#8, #15).
- [ ] **UI-06 — Accessibility and localization.** Meet agreed keyboard-only, screen-reader, focus, high-contrast and scaling requirements; externalize strings and test at least the supported locales. Record manual and automated evidence. Depends: PRD-01.
  *Status:* The sidebar exposes named tab rows and close buttons and a busy state during loading. Screen-reader review, high contrast, scaling and localization remain open.
- [ ] **UI-07 — Web compatibility.** Establish representative local fixtures and a maintained real-site compatibility matrix, including forms, media, downloads, authentication flows and common web APIs. Test privacy/security controls and document unsupported APIs without collecting real credentials. Depends: PRD-04.
  *Status:* Partial. YouTube playback and Media Source codecs were checked by hand (#15). No maintained matrix.

## P1 — Networking and optional product capabilities

- [ ] **NET-01 — Network ownership.** Define WebKit ownership of proxy, DNS, TLS and cookie behavior; implement only policies actually enforceable by the engine, document exceptions and test proxy/offline behavior. Depends: PRD-03, SEC-04.
  *Status:* WebKit owns networking and uses platform proxy and resolver defaults. The unused scaffolds were removed. Proxy behavior on supported packages remains unverified.
- [ ] **NET-02 — Content blocking lifecycle.** If included in v1, integrate `bitter-adblock` with requests, define filter provenance, refresh, integrity, failure and resource limits; test rules and allowlists. Otherwise defer transparently and remove false UI claims. Depends: SEC-06.
  *Status:* Partial. `bitter-adblock` converts lists, the browser downloads them over HTTPS in an ephemeral session, validates the format, rejects lists over the 8 MiB size limit, keeps the previous lists on failure, caps rules at 150,000 and refreshes every four days, with a per-site exception (#15). Missing: automated tests of network blocking and the per-site exception.
- [x] **DEV-01 — Developer tools.** Decide whether to expose WebKit inspector or implement additional tools; gate access appropriately and test opening, closing and production defaults. Remove placeholder claims. Depends: PRD-03.
  *Status:* WebKit inspector behind F12, Ctrl+Shift+I and Inspect Element, on by default and switchable in preferences, with an open and close smoke test (#15). WebKit Inspector is the supported developer tool.
- [ ] **CLI-01 — Command-line contract.** Either implement documented URI, profile, help, version and error semantics or remove the placeholder from shipped packages. Test invalid arguments and running-instance handling. Depends: DATA-01, PRD-03.
  *Status:* Partial. `bitter-ui` handles URIs, `--version` and `--help` and hands links to the running instance (#8, #10). `bitter-cli` has an argument parser, `--help`/`--version`, URI passthrough to `bitter-ui`, error reporting and tests, and Meson installs it alongside `bitter-ui` (#20, #26, #27); its behavior is documented in [Daily browsing](docs/daily-browsing.md#command-line-and-default-browser). Missing: profile selection, a decision on bare host names (today treated as files by GTK) and a running-instance test through `bitter-cli`.
- [ ] **EXT-01 — Extensions and credentials decision.** Explicitly decide support for extensions, password management, sync and account features; exclude them from v1 unless threat model, privacy design, consent and maintenance capacity are approved. Depends: PRD-01, PRD-02.
  *Status:* Not decided.

## P0 — Verification and observability

- [ ] **TEST-01 — Unit and integration suite.** Add behavior-driven tests for URL handling, settings, data paths, storage and failure paths; isolate HOME/XDG and use temporary databases. Add regression tests for every confirmed bug. Depends: DATA-01–03, UI-01.
  *Status:* Partial. 58 tests over URL handling, settings, storage, records, filter conversion, HTTPS upgrades and failure paths, with isolated profiles. Not every past bug has a regression test.
- [ ] **TEST-02 — GUI automation.** Establish reproducible isolated Wayland/X11 or virtual-display tests for launch, tabs, keyboard, internal pages, permissions and crash recovery using local fixtures. Do not use a real browsing profile. Depends: UI-01–06.
  *Status:* Partial. The Xvfb smoke test covers launch, tabs, workspaces, keyboard, internal pages, permissions, pop-ups, crash recovery, downloads and dialogs (#8, #10, #14, #15). No Wayland run.
- [ ] **TEST-03 — Security integration.** Automate hostile local origins, redirects, certificate failures, permission requests and crafted saved content; record actual browser/engine versions, results and limits. Depends: SEC-01–06.
  *Status:* Not started beyond the permission and pop-up checks in the smoke test.
- [ ] **TEST-04 — Coverage policy.** Measure per-crate and workspace line/branch coverage with documented exclusions; establish evidence-based ratcheting targets after a baseline and enforce no unjustified regressions. Coverage cannot substitute for TEST-02/03. Depends: TEST-01.
  *Status:* Partial. Coverage is measured in CI (46.77% line coverage on 2026-11-22, up from a 40.24% baseline on 2026-11-19; instrumented automated tests only); `docs/coverage.md` drafts the measurement, exclusion, and ratchet policy. Missing: per-crate/branch targets, documented exclusion list with owners, and enforced no-regression ratchet.
- [x] **TEST-05 — Continuous integration.** Run formatting, lint, unit/integration, coverage, workflow validation, CodeQL and dependency checks on protected PRs; test artifacts and caching safely for untrusted forks. Document required check ownership and failure response. Depends: TEST-01–04.
  *Status:* Formatting, Clippy, `cargo audit`, coverage, the display and smoke tests, CodeQL, dependency review and vendored source checks run on every PR and gate auto-merge (#5, #9, #11).
- [ ] **TEST-06 — Performance and soak.** Define hardware and repeatable startup, navigation, memory, tab-count and long-running benchmarks; investigate regressions and publish reproducible measurements. Depends: PRD-04.
  *Status:* Partial. The smoke harness measures startup and memory with 0, 50 and 100 saved tabs (#8). No soak test.

## P0 — Distribution and operations

- [ ] **OPS-01 — Reproducible packaging.** Build and install Flatpak and Arch packages in clean environments; verify runtime/SDK provenance, locked dependencies, icons, desktop registration, portals, upgrade and uninstall. Remove build-time network assumptions or document and mitigate them. Depends: PRD-01, DATA-01.
  *Status:* Partial. Offline Flatpak and Arch builds in CI, installed and used from release assets (#11, #12). Missing: upgrade and uninstall checks in clean environments.
- [x] **OPS-02 — Release pipeline and provenance.** Automate version consistency, clean build, SBOM, checksums and provenance/attestation; publish artifacts only from a protected reviewed tag with minimal permissions. Verify downloads and rollback procedure. Depends: OPS-01, TEST-05.
  *Status:* Signed protected tags, version checks, SBOM, checksums, provenance attestations and post-publish verification (#11, #12).
- [ ] **OPS-03 — Updates and vulnerability monitoring.** Define supported WebKit/OS/Flatpak runtime update cadence and response SLA; track sources not covered by Dependabot, validate compatibility and exercise emergency security releases. Depends: PRD-01, SEC-09.
  *Status:* Partial. Dependabot covers Cargo and Actions; the Flatpak runtime and WebKitGTK are updated by hand. No cadence or response target.
- [ ] **OPS-04 — Diagnostics and incident response.** Document privacy-preserving diagnostics, reproducible bug reports, crash handling, triage, maintainer ownership and security incident escalation; test recovery and release rollback. Depends: SEC-08, REL-01.
  *Status:* Partial. `docs/diagnostics-incident.md` is an initial draft grounded in current behavior and `SECURITY.md`. Missing: maintainer ownership/rotation, support/severity response targets, privacy-reviewed diagnostics bundle format, and tested rollback/incident drills.
- [ ] **OPS-05 — Community and support.** Keep installation, known limitations, compatibility, accessibility, security and contribution docs accurate; define maintenance capacity, issue triage and support expectations. Test published links and package instructions. Depends: PRD-01.
  *Status:* Partial. Install, usage, profile, release and security docs are kept current with each change. No support or triage policy.

## Release sequence

1. Complete PRD-01–04 and establish owners, scope and threat model. Create linked issues and identify dependencies before implementation.
2. Complete P0 security, data and browsing work alongside regression tests; no privacy or security control is advertised before it is effective and verified.
3. Finish GUI/security integration, packaging and independent security review. Record accepted exceptions with an owner and expiry; any unmet P0 gate blocks production.
4. Run a documented beta on supported configurations, resolve release blockers, then approve and publish a signed/attested production release only when PRD-04 gates are met. Keep rollback and security updates operational afterward.
