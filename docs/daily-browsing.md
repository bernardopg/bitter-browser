# Daily browsing

| Action | Shortcut |
| --- | --- |
| Open or close a tab | Ctrl+T or Ctrl+W |
| Reopen a recently closed tab | Ctrl+Shift+T |
| Move between tabs in the current workspace | Ctrl+Page Up or Ctrl+Page Down |
| Move the current tab earlier or later | Alt+Shift+Left or Alt+Shift+Right |
| Focus the address bar or command palette | Ctrl+L or Ctrl+K |
| Back, forward or reload | Alt+Left, Alt+Right or Ctrl+R |
| Cancel a page load or dismiss address suggestions | Escape |

Main and Second keep their own order and selected tab. Ctrl-click or
middle-click on a web link makes a background tab. Clicking a link that
requests a new window opens it in the current workspace. Address suggestions
can switch to an existing tab in either workspace. History and bookmarks are
queried locally after a 90 ms typing pause, with SQLite reads on a worker.
Entering a search and confirming sends it to the configured provider.

The toolbar shows loading progress with a Stop button while a page loads.
Connection and certificate failures show explanatory native pages with
retry. HTTP error responses still display the page sent by the server.

## Verification

Run the local WebKit smoke test and optionally collect startup samples.

```sh
cargo build -p bitter-ui --locked
xvfb-run -a -s '-screen 0 1440x1000x24' dbus-run-session -- \
  python3 scripts/daily-smoke.py target/debug/bitter-ui --benchmark-samples 5
```

The harness uses disposable profiles and loopback only. It checks navigation,
history suggestions, workspace synchronization, background link behavior,
reopening and reordering tabs, network error recovery, and session restoration
after terminating the browser process. It also checks that the 50-tab and
100-tab fixtures request only the selected page at launch. Time ends when
the window becomes visible. RSS covers the browser process and excludes
separate WebKit helper processes. These are X11 local measurements.

Local baseline on a Core i7-1255U with WebKitGTK 2.54.2 and a debug build.
Five fresh profiles per scenario ran serially under Xvfb. These timings
include GTK initialization and vary with host load.

| Saved tabs | Median to visible window | Range | Median browser RSS | Restored page requests per launch |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 339 ms | 322–1345 ms | 224.4 MiB | 0 |
| 50 | 811 ms | 572–843 ms | 220.3 MiB | 1 |
| 100 | 875 ms | 430–1931 ms | 222.9 MiB | 1 |
