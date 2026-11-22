# Agent instructions

This file is the single source of instructions for every coding agent working
on Bitter Browser (Claude Code, Codex, Cursor and others). `CLAUDE.md` only
imports it. Change the rules here, never in a copy.

Bitter Browser is a Linux web browser in Rust (GTK 4, libadwaita,
WebKitGTK 6). The workspace has six crates: `bitter-core` (profile, SQLite
history and bookmarks, settings), `bitter-adblock` (filter list conversion),
`bitter-ui` (the browser), `bitter-cli` (the `bitter-cli` launcher), and the
unused scaffolds `bitter-net` and `bitter-devtools`. Read
[README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md) and
[docs/ci-release.md](docs/ci-release.md) before changing build, CI or release
files.

## Checks before every push

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked
cargo audit                      # needs cargo-audit installed
xvfb-run -a dbus-run-session -- cargo test --workspace --locked
```

- WebKit tests need a display and a session bus. Run them under
  `xvfb-run -a dbus-run-session`, as CI does.
- Tests that create WebKit objects or use the global GLib main context must
  hold the test lock in their module (see `content_blocking.rs`). Parallel
  test threads otherwise panic or abort the test binary.
- Browser behavior changes also need the smoke test:
  `cargo build -p bitter-ui --locked && xvfb-run -a -s '-screen 0 1440x1000x24' dbus-run-session -- timeout 180s python3 scripts/daily-smoke.py target/debug/bitter-ui`.
- After changing `Cargo.lock`, run `scripts/update-flatpak-sources.sh` and
  commit `flatpak/cargo-sources.json`.
- Give each git worktree its own `target/` directory. A shared
  `CARGO_TARGET_DIR` mixes path dependencies from different checkouts and
  produces false compile errors.

## Code navigation: tokensave

The repository has a tokensave code graph in `.tokensave/` (ignored by git).
Use it before reading source files or grepping for symbols.

- Explore a question with `tokensave_context` (plain-English task). Pass the
  returned `seen_node_ids` as `exclude_node_ids` on the next call.
- Find a symbol by name with `tokensave_search`. With `literal: true` it
  finds exact text, such as an error message or every call site of a name.
- Use `tokensave_callers`, `tokensave_callees` and `tokensave_impact` for
  who calls what and what a change affects, and `tokensave_read` (modes
  `lines` and `map`) to read a slice or the symbol map of a file.
- Use grep only for a regular expression or for files that a literal search
  reports as `unscanned`. Read a file in full only when you are about to
  edit it.
- Check freshness with the `tokensave_status` tool. After you change code,
  run `tokensave sync` in the repository root. Track a long-lived branch with
  `tokensave branch add <branch>`. From another checkout or worktree, pass
  `graph_root: "/home/bitter/development/bitter-browser"` to query this
  index.
- Outside an agent: `tokensave tool search <name>` runs the same tools from
  the shell, `tokensave status` shows the index, `tokensave doctor` checks
  the installation.

## Memory: Headroom project store

All agents keep their memories about this project in the Headroom project
store, `.headroom/` at the repository root (`memory.db`,
`memory_vectors.db`, `memory_graph.db`; ignored by git). Do not write
project memories to an agent-specific store, such as Claude Code memory
files or Codex memories; other agents cannot read them.

- The `memory_*` tools (`memory_search`, `memory_save`, `memory_update`,
  `memory_delete`, `memory_list`) are injected by the Headroom proxy
  (`headroom.service`, started with `--memory`). With project storage the
  proxy keeps one store per project in
  `~/.headroom/memories/projects/bitter-browser-<hash>/`. On the owner's
  machine that directory is a symbolic link to `<repo>/.headroom`, so the
  tools and the `headroom memory` CLI (whose default is
  `./.headroom/memory.db`) use the same store. On a new machine, create the
  link once (check `~/.headroom/logs/proxy-*.log` for the
  `memory_project_resolved` path), then run
  `systemctl --user restart headroom.service`.
- Start agents from the repository root. A worktree resolves to a different
  project key; from a worktree, pass
  `--db-path <repo>/.headroom/memory.db` to the CLI and save memories from a
  session started at the root.
- Before saving, search: `memory_search` with a natural-language query (add
  `entities` to filter). Update an existing memory with `memory_update`
  instead of saving a duplicate; remove a wrong or obsolete one with
  `memory_delete` and a reason.
- Save with `memory_save`: `content`, `importance` (0.9–1.0 critical,
  0.7–0.8 important, 0.5–0.6 useful), and pre-extracted `facts`,
  `extracted_entities` and `extracted_relationships`. Each fact stands on
  its own and uses absolute dates.
- Save what the repository does not record: decisions and their reasons,
  pitfalls found while working (flaky tests, tool quirks), owner
  preferences, open problems and pointers to external resources. Do not save
  what git history, the docs or the code already state.
- Never save secrets, tokens, passwords or private browsing data.
- Inspect the store from the shell with `headroom memory list`,
  `headroom memory show <id>`, `headroom memory stats` and
  `headroom memory export --output <file>.json` (back up before
  `headroom memory prune` or `purge`).

## Branches and pull requests

- Branch from an up-to-date `main`. One topic per branch and pull request.
- `main` is protected: required checks must pass and the branch must be up
  to date with `main`. Owner pull requests get auto-merge enabled.
- When a pull request is `BEHIND`, merge `origin/main` into the branch and
  push. Do not rebase a branch that is already published.
- Commits, pull requests, tags, releases and docs carry no AI attribution:
  no `Co-Authored-By` trailer for an assistant and no "Generated with" line.

### Cleaning up merged branches (project default)

A branch whose pull request is merged into `main` must be deleted right after
the merge, both on GitHub and locally, together with any worktree that holds
it. Never keep merged branches around.

- GitHub: the *Branch cleanup and main verification* workflow
  (`.github/workflows/maintenance.yml`) runs every hour and deletes the head
  branch of every merged pull request, unless an open pull request still
  uses it or it has commits that are not on `main`. It is the safety net,
  not the plan. Run it at once with `gh workflow run maintenance.yml`, or
  delete by hand: `git push origin --delete <branch>`.
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

### Release check records

Every release gets a check record in `release-checks/vX.Y.Z.md`. The folder
is ignored by git: records stay on the release machine and are not
published. Write the record after the *Release* workflow succeeds, and keep
the records of earlier releases; the newest one is the template for the
next. A record has these sections:

1. **Header:** date, release URL, tag and its signing key (`git tag -v`),
   merge commit, release workflow run URL and the pull requests included.
2. **Automated checks:** a table with each command and its result:
   `check-version.py`, `check-flatpak-sources.py`,
   `appstreamcli validate --no-net` on the metainfo, `cargo fmt`,
   `cargo clippy`, `cargo audit`, the test run (passed, failed, ignored),
   `cargo llvm-cov` line coverage, the smoke test, and, on the downloaded
   assets (`gh release download vX.Y.Z` into an empty directory),
   `sha256sum -c` and `gh attestation verify`.
3. **Asset checksums:** the contents of `bitter-browser-X.Y.Z-sha256.txt`.
4. **Problems found during the release:** each bug, flaky check or process
   gap, with its state (fixed in which pull request, or open and where it is
   tracked). Save the open ones to Headroom memory as well.
5. **Manual test checklist:** what the owner tests on the final package,
   grouped by install, each new feature, each fix and general regression.
   The owner marks items `[x]` with the date and environment (Flatpak or
   Arch, Wayland or X11).

Give the owner the record's path and the manual checklist when the release
is done.
