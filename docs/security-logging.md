# Security, logging, and telemetry posture (draft)

This document describes the current security/logging posture as implemented in
the codebase. It is a draft for SEC-08 and should be revised when behavior or
policy changes. It does not claim an independent security review, audit, or
production telemetry program.

## Telemetry

- No product telemetry, analytics, crash reporting service, or remote browsing
  data collection is implemented in the application code.
- The repository contains CI and release automation metadata, but no
  user-facing telemetry policy or opt-in/opt-out telemetry flow.
- Before adding any production telemetry, define a user-facing policy, data
  minimization, retention, access control, and explicit consent/expectations.

## Logging

- The GUI uses `tracing` and `tracing-subscriber` for runtime diagnostics.
- The CLI prints user-facing errors/help/version text to stdout/stderr.
- Logs are intended for operational/debug diagnostics, not as a data collection
  channel.

## Sensitive data handling expectations

- Do not log credentials, passwords, form data, cookies, session tokens, or
  full browsing history.
- Avoid logging raw URLs when they may contain secrets; prefer origins, page
  classes, or redacted identifiers where practical.
- Treat download paths, file names, and site identifiers as potentially
  sensitive; include them only when needed for actionable diagnostics.
- Crash reports or future diagnostics bundles should be privacy-preserving and
  explicitly reviewed for browsing-data leakage before being enabled.

## Secrets

- No browser password manager or credential sync is implemented.
- If secrets are stored in the future, use an appropriate OS secret service and
  document denial/unavailable behavior. Do not store secrets in plain config or
  logs.

## Current gaps

- There is no formal log redaction/audit pass over all `tracing` call sites.
- There is no user-facing diagnostics bundle or crash-report policy.
- Security-sensitive logging expectations are not yet enforced by automated
  checks.
