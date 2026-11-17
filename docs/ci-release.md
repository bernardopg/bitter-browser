# CI, coverage and releases

CI runs on pull requests and main using a GitHub-hosted Ubuntu runner. It installs the GTK, libadwaita and WebKitGTK development packages, checks formatting and Clippy, and runs `cargo llvm-cov` across workspace targets. The LCOV artifact expires after 14 days. Coverage measures instrumented automated tests; it does not prove graphical interaction, integration with live websites, or 100% coverage. No coverage threshold has been set.

CodeQL analyzes Rust and GitHub Actions without a build. Dependabot checks Cargo and GitHub Actions weekly; it does not track Flatpak runtimes or system packages, which maintainers must review separately. Actions are pinned to commit SHAs and Dependabot can update them. Labeling uses a metadata-only `pull_request_target` workflow: never add checkout or execution of PR code to that workflow.

Workflows use the automatically supplied, scoped `GITHUB_TOKEN`; no personal access token, third-party coverage service, or repository secret is required. Fork PR tokens are restricted. Local hooks are optional and must not be treated as security enforcement.

For a release, update workspace, Meson and PKGBUILD versions together, run checks, build and validate the Flatpak, merge the reviewed PR, and create a tag and GitHub release pointing at the merged commit. Attach the bundle and checksum. The repository does not currently automate publishing or signing; verify the release and assets after upload.
