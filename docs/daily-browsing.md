# Daily browsing

| Action | Shortcut |
| --- | --- |
| Open or close a tab | Ctrl+T or Ctrl+W |
| Reopen a recently closed tab | Ctrl+Shift+T |
| Move between tabs in the current workspace | Ctrl+Page Up or Ctrl+Page Down |
| Move the current tab earlier or later | Alt+Shift+Left or Alt+Shift+Right |
| Focus the address bar or command palette | Ctrl+L or Ctrl+K |
| Back, forward or reload | Alt+Left, Alt+Right or Ctrl+R |
| Discard an address draft or stop a page load | Escape |
| Find in the page, next or previous match | Ctrl+F, Ctrl+G or Ctrl+Shift+G |
| Zoom in, zoom out or reset zoom | Ctrl+Plus, Ctrl+Minus or Ctrl+0 |
| Enter or leave full screen | F11 (Escape also leaves) |
| Show downloads | Ctrl+J |
| Print the page | Ctrl+P |
| Open or close the Web Inspector | F12 or Ctrl+Shift+I |
| Clear browsing data | Ctrl+Shift+Delete |
| Show or hide the sidebar | F9 |
| Preferences | Ctrl+, |

Main and Second keep their own order and selected tab. Closing the last tab
in the active workspace creates a fresh New Tab there. Ctrl+Shift+T reopens a
closed page at its previous position and selects it in its original workspace.
The other workspace keeps its tabs and selection.
 Ctrl-click or
middle-click on a web link makes a background tab. Clicking a link that
requests a new window opens it in the current workspace. Windows a page opens
with `window.open`, such as sign-in and payment pop-ups, open as the next tab
and keep their connection to the page that opened them; when they close
themselves, the browser returns to that page. Pages cannot open windows
without a click or key press.

The context menu opens links, images and media in a new or background tab,
and searches selected text with the configured provider. Hovering a link
shows its address in the page's bottom corner. Address suggestions
can switch to an existing tab in either workspace. History and bookmarks are
queried locally after a 90 ms typing pause, with SQLite reads on a worker.
Entering a search and confirming sends it to the configured provider.
Ctrl+L focuses and selects the address. Enter commits the draft. Escape in
the address bar restores the active tab's address and dismisses suggestions,
including when no suggestions are shown. Escape outside an address edit
can stop a pending load.
Links opened from other applications open in a new tab of the running
window. Only `http`, `https` and `file` addresses are accepted.

The toolbar shows loading progress with a Stop button while a page loads.
Connection and certificate failures show explanatory native pages with
retry. HTTP error responses still display the page sent by the server.

## Pages, zoom and full screen

Find in page highlights every match, counts up to 1,000 matches and wraps
around the page. Enter and Shift+Enter move between matches; Escape closes
the bar. Switching tabs closes it too. Zoom steps between 30% and 300% and
is remembered for each site; Ctrl+0 returns the site to the default zoom set
in Preferences. Full screen hides the
sidebar and toolbar. Videos and pages that request full screen use the same
mode, and Escape returns control to the page in that case. The window keeps
its size and maximized state between launches when the GSettings schema is
installed, as it is in Flatpak and Meson installations. Tabs show the site
icon; restored tabs show the stored icon before their page loads.

## Permissions

Sites ask before they show notifications, use your location, camera or
microphone, or read the clipboard. The question appears in a bar above the
page and the answer is remembered for that site (its scheme, host and port).
Screen sharing is asked every time. Leaving the page withdraws unanswered
questions. A site that asks from a background tab shows a notice. Change or
forget answers from the address bar icon, or in Preferences → Permissions.
Pages can read the remembered answer through `navigator.permissions`.

## Privacy

**Content blocking** is on by default. A small built-in list works at once;
EasyList and EasyPrivacy are downloaded from easylist.to into the profile,
converted to WebKit content blocker rules and compiled by WebKit, which
caches the result. Lists refresh every four days and from Preferences →
Privacy → Update Now. A failed update keeps the previous lists. Filters
that WebKit cannot express (regular expressions, scriptlets, `$csp`,
`$redirect` and similar) are skipped rather than approximated. Turn blocking
off for a site from the address bar icon.

**HTTPS-only mode** (off by default) loads `http:` pages over HTTPS. When a
site has no secure version, or sends the browser back to HTTP, a warning
page offers to try again or continue over HTTP for that site until the
browser closes. Local names (`localhost`, single-label hosts, `.local`,
`.lan`, `.home.arpa`, `.internal`, `.test`) and private or loopback
addresses stay on HTTP.

**Tracking and cookies.** Cross-site tracking prevention (WebKit's
Intelligent Tracking Prevention) is on by default, and so is blocking
third-party cookies, which WebKit also does by default.

**Clear browsing data** removes history, cookies and site data, cached
files and the download list, for the last hour, day, week, four weeks or all
time. The download list is always cleared completely. WebKitGTK cannot pick
cookies by age, so for a time range the browser removes the cookies of the
sites you visited in that range; "All time" removes every cookie. To remove
one page from history, open History (Ctrl+H) and press × on its row.

Spell checking follows your system languages and needs the matching
dictionaries (for example `hunspell-en_us`). Developer tools can be turned
off in Preferences → Advanced.

## Video

YouTube and other sites play through GStreamer. Install `gst-plugins-good`
(VP9, WebM, Opus), `gst-plugins-bad` and `gst-libav` (H.264, AAC); add
`gst-plugin-va` for hardware decoding on Intel and AMD graphics. Encrypted
media (Widevine DRM) is not available in WebKitGTK, so Netflix, Disney+,
Prime Video and Spotify's web player do not play.

## Downloads

Files the engine cannot display, and responses the server marks as
attachments, are saved to the XDG download directory, usually `~/Downloads`.
Names suggested by the server are reduced to a single file name. Path
separators, control characters and leading dots are removed, and an existing
file is never replaced: a second `report.pdf` is saved as `report (1).pdf`.
The Downloads button appears after the first download. It lists progress
and lets you cancel, open the file or show it in its folder. Finished downloads
stay listed after a restart until you clear them.

## Editing bookmark titles

Open a bookmarked page and press Ctrl+Shift+D, or choose **Edit Bookmark Title**
from the command palette. Save changes the title shown in bookmarks and search
while keeping the address and original sort order. Escape closes the dialog and
discards the draft.

## Command line and default browser

```sh
bitter-ui https://example.org        # opens in the running window, if any
bitter-ui --version
bitter-ui --help
bitter-cli https://example.org       # launcher with the same behavior
bitter-cli --version
xdg-settings set default-web-browser br.com.bebitterbebetter.browser.Bitter.desktop
```

`bitter-cli` is a small launcher installed next to `bitter-ui`. It accepts
`-h`/`--help` and `-v`/`--version`, rejects unknown options with exit code 2,
and passes every other argument to `bitter-ui` (everything after `--` is
passed unchanged). It runs the `bitter-ui` next to it, or the program named
by `BITTER_BROWSER_BIN`, and returns its exit code. Both use the same
default profile. The supported launcher contract uses one profile.

Give full addresses with their scheme. A bare name such as `example.org` is
treated as a file in the current directory, as with other GTK
applications.

Under Flatpak, use `flatpak run br.com.bebitterbebetter.browser.Bitter` with the same
arguments. **Preferences > General > Default browser > Make Default** sets the
default browser from inside the browser; desktop environments can also set it
in their settings. [Distribution](distribution.md#make-bitter-browser-the-default-browser)
lists the setting on each desktop.

## Verification

Run the local WebKit smoke test and optionally collect startup samples. Install
OpenSSL to generate the untrusted certificate fixture for the test.

```sh
cargo build -p bitter-ui -p bitter-cli --locked
scripts/headless-session.sh --screen 1440x1000x24 -- \
  python3 scripts/daily-smoke.py target/debug/bitter-ui --launcher target/debug/bitter-cli --benchmark-samples 5
```

The harness uses disposable profiles and loopback only. It checks navigation,
history suggestions, workspace synchronization, background link behavior,
reopening and reordering tabs, network error recovery, session restoration
after terminating the browser process, links handed over by a second
launch, downloads with hostile and repeated names, find in page, zoom, full
screen, favicons, window size persistence and the command-line options,
pop-ups that keep `window.opener` and close themselves, a notification
permission that is remembered and reported to the page, the context menu,
the Web Inspector, element hiding by the built-in filter list and the site,
clearing and permissions panels. Filter list downloads are disabled in the
test through `BITTER_OFFLINE_FILTER_LISTS`. It
also checks that the 50-tab and
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
