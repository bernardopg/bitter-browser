# Production release gates

Policy revision 1. The release owner records the candidate commit, artifact
digests, package/engine versions, display backend, evidence links and reviewers
in the release check record. Test installed candidates on the platform pairs
in the [support contract](support.md). Record the Xvfb CI run separately from
native Arch and Flatpak sessions.

| Gate | Acceptance evidence for the candidate |
| --- | --- |
| Security and privacy | Dependency audit and CodeQL pass. An independent reviewer signs off on the [threat model](threat-model.md) and relevant fixes. All P0 security regressions pass, including isolation, TLS rejection, origin-scoped permissions and no unexpected browsing data in diagnostics. |
| Browsing and compatibility | CI passes the isolated smoke test. In each supported package/display pair, verify navigation, tabs, forms, uploads/downloads, session restore, a sign-in pop-up with test accounts and media playback. Record each result and engine version. |
| Accessibility | Keyboard-only navigation reaches address, tabs, downloads, settings and permission controls with visible focus. Orca reports meaningful names and state. At 200% scale and in high contrast the same actions remain usable. Run on native Wayland and X11. |
| Performance | On one recorded machine, engine version and rendering backend, compare five startup samples and memory after restoring 0, 50 and 100 saved tabs with the last approved release. Investigate a median startup or post-restore memory increase above 20% before sign-off. Run a one-hour mixed browsing soak with a disposable profile and record crashes, hangs and memory at the start and end. |
| Recovery and packaging | Install, upgrade and uninstall on clean Arch and Flatpak environments. Verify bookmarks, settings and session recovery after an interrupted launch or write, and recover the prior release from its signed artifact and a backup profile. Record data integrity, portals and signed artifact verification. |
| Support | Identify the maintainer driving triage for the release, the supported runtime update path, and the private disclosure contact. Confirm that users can reach the documented reports channel and that the maintainer can reproduce a report from sanitized evidence. |

A missing result, failing acceptance check or open P0 defect blocks production
approval. Coverage can inform a review alongside tests of actual behavior.
Issues outside P0 need a recorded impact, owner, mitigation and review date
before approval. The release owner makes the final decision after the
independent security review, using the [release procedure](ci-release.md) and
the [roadmap](../TASKS.md).
