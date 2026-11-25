# Agent instructions

This file is the single source of instructions for every coding agent working
on Bitter Browser (Claude Code, Codex, Cursor and others). `CLAUDE.md` only
imports it. Change the rules here, never in a copy.

Bitter Browser is a Linux web browser in Rust (GTK 4, libadwaita,
WebKitGTK 6). The workspace has four crates: `bitter-core` (profile, SQLite
history and bookmarks, settings), `bitter-adblock` (filter list conversion),
`bitter-ui` (the browser), and `bitter-cli` (the launcher). Read
[README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md) and
[docs/ci-release.md](docs/ci-release.md) before changing build, CI or release
files.

## Checks before every push

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked
cargo audit                      # needs cargo-audit installed
scripts/headless-session.sh -- cargo test --workspace --locked
```

- WebKit tests need a display and a session bus. Run them through
  `scripts/headless-session.sh`, as CI does, never through a bare
  `xvfb-run -a dbus-run-session`. See
  [Display tests](#display-tests-headless-session).
- Tests that create WebKit objects or use the global GLib main context must
  hold the test lock in their module (see `content_blocking.rs`). Parallel
  test threads otherwise panic or abort the test binary.
- Browser behavior changes also need the smoke test:
  `cargo build -p bitter-ui --locked && scripts/headless-session.sh --screen 1440x1000x24 -- timeout 180s python3 scripts/daily-smoke.py target/debug/bitter-ui`.
- After changing `Cargo.lock`, run `scripts/update-flatpak-sources.sh` and
  commit `flatpak/cargo-sources.json`.
- Give each git worktree its own `target/` directory. A shared
  `CARGO_TARGET_DIR` mixes path dependencies from different checkouts and
  produces false compile errors.

## Display tests (headless session)

Run every command that needs a display or a session bus (`cargo test`,
`cargo llvm-cov`, the ignored preferences test, `scripts/daily-smoke.py`)
through `scripts/headless-session.sh`:

```sh
scripts/headless-session.sh -- cargo test --workspace --locked
GTK_A11Y=none GDK_BACKEND=x11 GSK_RENDERER=cairo \
  scripts/headless-session.sh -- \
  cargo test -p bitter-ui --locked settings_widgets_save_restore_and_report_errors -- --ignored
cargo build -p bitter-ui --locked
scripts/headless-session.sh --screen 1440x1000x24 -- \
  timeout 180s python3 scripts/daily-smoke.py target/debug/bitter-ui
```

The script runs the same pair as before (Xvfb plus a new session bus from
`dbus-run-session`) and adds two protections for runs on a developer
desktop:

- **A private, temporary `XDG_RUNTIME_DIR`** (mode 0700, removed at exit).
  `dbus-run-session` creates a new bus, but the services that bus activates
  (`xdg-document-portal`, `xdg-desktop-portal` and its backends) inherit
  `XDG_RUNTIME_DIR`. On a desktop that is `/run/user/<uid>`, shared with the
  real session. The test session's document portal took over
  `/run/user/<uid>/doc` and unmounted it when the test ended. The desktop's
  own `xdg-document-portal.service` then exited with status 21 and stayed
  failed, so Flatpak apps lost file access until the next login. This
  happened on every local smoke run (found on 2026-11-22).
  `scripts/daily-smoke.py` already gives the browser process its own
  runtime directory, but that never reached the bus-activated portals.
- **No desktop session variables** (`WAYLAND_DISPLAY`,
  `XDG_CURRENT_DESKTOP`, `HYPRLAND_INSTANCE_SIGNATURE` and others). With
  `XDG_CURRENT_DESKTOP=Hyprland` inherited, the test bus started
  `xdg-desktop-portal-hyprland`, which segfaults without a compositor and
  left a coredump on every run.

CI runners have no desktop session, so CI never showed either problem. CI
uses the script anyway, so local and CI runs are identical.

- A crash of `bitter_ui-<hash>` (the test binary) or `bitter-ui` under
  `target/` during these runs is a failure to investigate here, not system
  noise.
- The script cleans up after itself. If a run is killed with `SIGKILL`, a
  `/tmp/bitter-runtime.*` directory can remain. Unmount the `doc` mount
  inside it with `fusermount3 -u` before you remove it, because its files
  map to real user files.
- Check that a run left the desktop intact:
  `systemctl --user --failed` lists no `xdg-*portal*` unit, and
  `mountpoint /run/user/$(id -u)/doc` still reports a mount point.

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

- Three ways reach the same store: the `memory_*` tools injected by the
  Headroom proxy (`headroom.service`, `--memory`), the `headroom_memory` MCP
  server (`mcp__headroom_memory__memory_*`, via
  `~/.local/bin/headroom-memory-mcp`), and the `headroom memory` CLI run
  from the repository root. Worktrees and subdirectories resolve to the
  main repository's `.headroom/`.
- The proxy keeps a store per directory in
  `~/.headroom/memories/projects/`. A `SessionStart` hook runs
  `headroom-project-memory`, which links that directory to `.headroom/` and
  merges anything already stored there. Run it by hand from the repository
  root, or check a path with `headroom-project-memory --check .`, if memory
  tools write somewhere else. The full machine setup is in the owner's
  global rule `~/claude-env/rules/headroom-memory.md` (also in
  `~/.codex/AGENTS.md`).
- Only `headroom.service` runs a proxy on port 8791. Do not run
  `headroom init`, `headroom install apply` or `headroom init hook ensure`:
  they start a second proxy without memory that takes the port.
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
  Import with `headroom memory import <file>.json --force`, then run
  `headroom-project-memory` so the vectors are indexed.

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

## Packaging and distribution

The full guide is [docs/distribution.md](docs/distribution.md).

- The application ID is `br.com.bebitterbebetter.browser.Bitter` (domain
  `browser.bebitterbebetter.com.br`, a subdomain of the owner's site). Use it
  for the desktop file, metainfo, GSettings schema, icons and Flatpak
  manifest. `com.bitterbrowser.Browser` is the old ID: `bitterbrowser.com`
  belongs to another product, so never reuse it.
- Packaging lives in `packaging/` (`linux/`, `debian/`, `rpm/`, `arch/`,
  `arch-bin/`) and the root `PKGBUILD` (`-git`). The *Packages* workflow
  builds and installs every format on pull requests that touch packaging;
  run the same scripts locally in Docker containers (examples in the guide).
- A version bump also changes `packaging/arch-bin/PKGBUILD` and
  `packaging/rpm/bitter-browser.spec`; `scripts/check-version.py` checks them.
- The brand (logo, app icon, colors, type, social images, site fonts) is
  generated by `branding/build.py`; change the code there and commit the
  outputs, never edit exported files. `branding/README.md` is the brand book.
  For a one-off icon change outside the brand build, `scripts/render-icons.sh`
  renders the PNG sizes from the SVG.
- The website lives in `site/`; `scripts/build-site.sh` assembles it with the
  brand assets, the media kit zip and the brand book PDF, and the *Website*
  workflow publishes it on pushes to `main`
  (`scripts/deploy-site.sh` by hand, reading `.env`). The site must not load
  anything from third parties; the workflow checks it.
- Release secrets (AUR account and SSH key, pacman signing key) are GitHub
  Actions secrets. The owner keeps the values in `.env` (ignored by git;
  `.env.example` lists the names). Never commit `.env`, print its values or
  copy them into logs, issues or memories.
- Flathub forbids AI-written Flathub manifests and AI-opened or automated
  submission pull requests. Agents prepare the upstream metadata only; the
  owner writes and submits the Flathub manifest.
- No AppImage: WebKitGTK 6.0 confines its helper processes with bubblewrap
  at a compiled-in path, and the project does not ship a browser with that
  sandbox turned off. The reasons are in the guide.

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
