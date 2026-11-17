# Contributing

Thank you for helping improve Bitter Browser. For substantial changes, open an issue first to discuss the problem and scope. Search existing issues and pull requests before filing a new one. Keep changes focused and describe observable behavior and limitations.

## Setup and checks

Follow the dependencies and commands in the [README](README.md). Use a branch based on `main`. Before opening a PR, run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked`, and `cargo test --workspace --locked`. Add meaningful tests for behavior changes. CI also collects workspace coverage; an unexecuted GUI path is not proven by a successful unit test.

Optionally enable the local pre-commit hook with `git config core.hooksPath .githooks`. This runs formatting and staged whitespace checks; it does not replace CI. Disable it with `git config --unset core.hooksPath` if needed.

Do not commit credentials, private profiles, browser history, or generated build artifacts. Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md). By submitting a contribution you agree to its distribution under the project's MPL-2.0 license.
