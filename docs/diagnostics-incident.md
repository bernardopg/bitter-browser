# Diagnostics and incident response basics (draft)

This document is an initial draft for OPS-04. It describes practical,
privacy-preserving diagnostics and incident handling expectations grounded in
current repository behavior. It is not a completed support, SRE, or security
incident program.

## Diagnostics

- Reproducible bug reports should include: Bitter version, package/build type,
  OS/distribution, display server/compositor, WebKitGTK version where relevant,
  steps to reproduce, expected result, actual result, and whether the issue
  reproduces in a clean profile.
- Diagnostics should be privacy-preserving by default. Do not ask users to
  share passwords, cookies, session data, or unrelated browsing history.
- Logs, screenshots, and recordings may contain sensitive site content; users
  should redact them before sharing.
- If future automated diagnostics are added, they must have explicit scope,
  redaction, retention, and user consent/documentation.

## Crash and failure handling

- The browser has recovery pages for some navigation/certificate/process
  failures and reports some storage failures through UI messages.
- There is no centralized crash-report collection or incident dashboard.
- Bug triage should distinguish product bugs, packaging/environment issues,
  WebKitGTK/runtime issues, and security-sensitive reports.

## Incident response basics

- Security vulnerabilities should follow `SECURITY.md` private reporting
  channels; do not open public issues for suspected vulnerabilities.
- Maintainers should record: report source, affected versions, reproduction,
  severity assessment, mitigation, fix, release, and user communication.
- A security fix release should include versioned notes, checksums/SBOM/
  provenance where applicable, and a clear rollback/upgrade path.
- Post-incident follow-up should capture regression tests or documentation
  updates needed to prevent recurrence.

## Current gaps

- No defined maintainer ownership/rotation for triage or incidents.
- No documented support SLA or severity/response targets.
- No tested rollback drill or incident tabletop exercise is recorded.
- No privacy-reviewed diagnostics bundle format exists.
