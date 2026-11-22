# Agent instructions

Bitter Browser is a Linux web browser in Rust (GTK 4, libadwaita,
WebKitGTK 6). The workspace has four crates: `bitter-core` (profile, SQLite
history and bookmarks, settings), `bitter-adblock` (filter list conversion),
`bitter-ui` (the browser) and `bitter-cli` (the `bitter-cli` launcher).
Read [README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md) and
[docs/ci-release.md](docs/ci-release.md) before changing build, CI or release
files.

## Checks before every push

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked
cargo audit
xvfb-run -a dbus-run-session -- cargo test --workspace --locked
```

- WebKit tests need a display and a session bus. Run them under
  `xvfb-run -a dbus-run-session`, as CI does.
- Browser behavior changes also need the smoke test:
  `cargo build -p bitter-ui --locked && xvfb-run -a -s '-screen 0 1440x1000x24' dbus-run-session -- timeout 180s python3 scripts/daily-smoke.py target/debug/bitter-ui`.
- After changing `Cargo.lock`, run `scripts/update-flatpak-sources.sh` and
  commit `flatpak/cargo-sources.json`.
- Give each git worktree its own `target/` directory. A shared
  `CARGO_TARGET_DIR` mixes path dependencies from different checkouts and
  produces false compile errors.

## Branches and pull requests

- Branch from an up-to-date `main`. One topic per branch and pull request.
- `main` is protected: required checks must pass and the branch must be up
  to date with `main`. Owner pull requests get auto-merge enabled.
- When a pull request is `BEHIND`, merge `origin/main` into the branch and
  push. Do not rebase a branch that is already published.

### Cleaning up merged branches (project default)

A branch whose pull request is merged into `main` must be deleted, both on
GitHub and locally, together with any worktree that holds it. Never keep
merged branches around.

- GitHub: the *Branch cleanup and main verification* workflow
  (`.github/workflows/maintenance.yml`) runs every hour and deletes the head
  branch of every merged pull request, unless an open pull request still
  uses it or it has commits that are not on `main`. Run it at once with
  `gh workflow run maintenance.yml`. Delete by hand when needed:
  `git push origin --delete <branch>`.
- Local, after each merge:

  ```sh
  git fetch --prune origin
  git switch main && git merge --ff-only origin/main
  git worktree remove <path-of-the-branch-worktree>   # if any
  git branch -d <branch>
  ```

  `git branch -d` refuses a branch that is not merged. Use `-D` only after
  `git cherry origin/main <branch>` prints no `+` lines, which means every
  commit is already on `main`.
- Branches of closed, unmerged pull requests are not deleted automatically.
  Check them with `git cherry` and ask the owner before deleting them.

## Releases

Follow [docs/ci-release.md](docs/ci-release.md#releases):
bump every version file, confirm with `python3 scripts/check-version.py`,
merge, then tag the merge commit on `main` with `git tag -s vX.Y.Z` and push
the tag. The *Release* workflow builds and publishes the assets.
