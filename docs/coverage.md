# Coverage policy (draft)

This document describes how automated test coverage is measured for Bitter
Browser and how the numbers should be interpreted. It is a draft policy for
TEST-04; it does not define a production quality gate by itself.

## What is measured

- Tool: `cargo llvm-cov`.
- Scope: workspace Rust tests run by CI (`cargo llvm-cov --workspace --all-targets --locked --lcov --output-path lcov.info`).
- Artifact: `lcov.info` uploaded by CI with a 14-day retention.
- Current measurement: 46.77% line coverage of instrumented automated tests
  (2026-11-22, v0.4.0), up from the 40.24% baseline of 2026-11-19.

## What is not measured

- The Xvfb smoke test is not instrumented by the current coverage run.
- GTK/WebKit display-dependent behavior is partially exercised by smoke and
  display tests, but many UI paths are difficult to measure meaningfully in
  headless coverage runs.
- Coverage does not prove graphical correctness, accessibility, live-website
  compatibility, security properties, or absence of regressions.

## Exclusions and limitations

- No blanket source exclusion policy is committed yet. Any future exclusion
  should be documented with a reason, owner, and review date.
- Generated files and vendored dependencies should not be counted as project
  coverage targets.
- Display-dependent UI code may need separate evidence (smoke tests, manual
  checks, screenshots, or targeted integration tests) rather than line coverage
  alone.

## Ratchet policy

- No coverage percentage alone satisfies a release or quality gate.
- Once a baseline is accepted for a crate or workspace area, PRs should not
  cause unjustified coverage regressions in that measured scope.
- Increases should be ratcheted deliberately: update the recorded baseline only
  when the measurement method is comparable.
- New behavior should prefer behavior-driven tests; coverage is a supporting
  signal, not the goal.

## Updating this policy

When changing measurement commands, scopes, exclusions, or baselines, update
this document, `docs/ci-release.md`, and the relevant `TASKS.md` status in the
same change.
