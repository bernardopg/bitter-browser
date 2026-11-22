# Claude Code instructions

@AGENTS.md

The rules in [AGENTS.md](AGENTS.md) apply in full. The most important ones:

- Run the checks in AGENTS.md before every push. WebKit tests run under
  `xvfb-run -a dbus-run-session`.
- Delete every branch that is merged into `main`, on GitHub and locally,
  together with its worktree. Do this right after the merge; the hourly
  *Branch cleanup and main verification* workflow is the safety net, not the
  plan. Use `git branch -d`, and `-D` only when `git cherry origin/main
  <branch>` shows no `+` lines.
- Ask the owner before deleting a branch whose pull request was closed
  without merging.
