# CI, coverage and releases

CI runs on pull requests and main using a GitHub-hosted Ubuntu runner. It installs the GTK, libadwaita and WebKitGTK development packages, checks formatting and Clippy, and runs `cargo llvm-cov` across workspace targets. The LCOV artifact expires after 14 days. Coverage measures instrumented automated tests; it does not prove graphical interaction, integration with live websites, or 100% coverage. No coverage threshold has been set. See [Coverage policy (draft)](coverage.md) for measurement scope, exclusions, and ratchet guidance.

CodeQL analyzes Rust and GitHub Actions without a build. Dependabot checks Cargo and GitHub Actions weekly; it does not track Flatpak runtimes or system packages, which maintainers must review separately. Actions are pinned to commit SHAs and Dependabot can update them. Labeling uses a metadata-only `pull_request_target` workflow: never add checkout or execution of PR code to that workflow.

Workflows use the automatically supplied, scoped `GITHUB_TOKEN`; no personal access token or third-party coverage service is required. Only the *Release* workflow reads repository secrets: the AUR account and SSH key and the pacman signing key, listed in [Distribution](distribution.md#secrets). Pull requests never receive them. Fork PR tokens are restricted. Local hooks are optional and must not be treated as security enforcement.

Pull requests opened by the repository owner from a branch of this repository get auto-merge enabled with a merge commit. GitHub merges them only after all required checks pass and the branch is up to date with `main`; draft pull requests, forks and Dependabot are excluded. Disable it for one pull request with `gh pr merge --disable-auto`. Merges made with `GITHUB_TOKEN` start no workflows: GitHub sends no `closed` or `push` event for them, and repository auto-delete skips them. The hourly *Branch cleanup and main verification* workflow (`maintenance.yml`, also runnable with `gh workflow run maintenance.yml`) covers both gaps. It deletes the head branch of every merged pull request whose tip is already on `main` and that no open pull request uses, and it starts CI and CodeQL for a `main` commit that has no run yet. Merged branches are always deleted, on GitHub and locally; see [AGENTS.md](../AGENTS.md#cleaning-up-merged-branches-project-default).

## Packaging

The Flatpak manifest builds without network access. `flatpak/cargo-sources.json`
vendors every crate in `Cargo.lock`; after changing dependencies, run
`scripts/update-flatpak-sources.sh` and commit the result. The CI job
*Flatpak sources* reports when the file is out of date. Build and install
locally with:

```sh
flatpak-builder --user --install --force-clean build-dir flatpak/br.com.bebitterbebetter.browser.Bitter.json
```

`packaging/arch/PKGBUILD` builds a tagged release from its source archive,
`packaging/arch-bin/PKGBUILD` repackages the portable build, and the root
`PKGBUILD` builds `bitter-browser-git` from `main`. The source recipes fetch
crates in `prepare()`, build offline and run the display-free tests in
`check()`. The `.deb`, `.rpm`, portable archive, AUR recipes and pacman
repository come from `packaging/`; the *Packages* workflow builds and
installs all of them on pull requests that touch packaging. See
[Distribution and desktop integration](distribution.md).

## Releases

1. Update the version in `Cargo.toml` (then `cargo update --workspace`),
   `meson.build`, `packaging/arch/PKGBUILD`, `packaging/arch-bin/PKGBUILD`,
   `packaging/rpm/bitter-browser.spec` (`Version` and a `%changelog` entry),
   the newest `<release>` in the
   AppStream metadata, the README bundle name and `CHANGELOG.md`.
   `python3 scripts/check-version.py` confirms they agree.
2. Merge the pull request, then tag the merge commit on `main` and push the
   tag: `git tag -s vX.Y.Z && git push origin vX.Y.Z`.
3. The *Release* workflow runs a chain of jobs; each one needs the previous
   one to pass. *check* confirms the versions, that the tag is on `main` and
   that the vendored sources are current. *packages* (the *Packages*
   workflow) builds the source archive, the `.deb`, portable archive, `.rpm`,
   Arch packages, signed pacman repository and Flatpak bundle, installs each
   package on its distributions and drives the installed browser.
   *publish* generates an SPDX SBOM and a checksum file, attests build
   provenance for every asset and publishes the release with the notes from
   `CHANGELOG.md` and `packaging/release-install-notes.md`, then verifies the
   published checksums and attestations. *verify* installs the published
   release with `install.sh` and the pacman repository on Debian, Ubuntu,
   Fedora, Arch and openSUSE. *aur* then pushes `bitter-browser`,
   `bitter-browser-bin` and `bitter-browser-git` to the AUR and checks the
   versions the AUR reports.
4. The workflow already verifies the assets; spot-check one download with
   `sha256sum -c` and `gh attestation verify`, and the AUR pages.
5. Write the release check record in `release-checks/vX.Y.Z.md` (ignored by
   git) with the checks, checksums, problems found and the manual test
   checklist; see [AGENTS.md](../AGENTS.md#release-check-records).

Tags matching `v*` are protected by a repository ruleset: they cannot be
updated or deleted, and only repository administrators can create them.
