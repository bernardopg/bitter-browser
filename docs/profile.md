# Profiles and local data

Bitter Browser opens one default Linux profile per XDG directory set. It uses
the effective XDG directories provided to the process, including Flatpak's
application-specific directories. Standard XDG home-relative defaults apply
when the respective variable is unset.

| Location | Contents |
| --- | --- |
| `$XDG_CONFIG_HOME/bitter-browser/` | `config.toml` |
| `$XDG_DATA_HOME/bitter-browser/` | `history.sqlite` and `bookmarks.sqlite` |
| `$XDG_DATA_HOME/bitter-ui/` | Persistent WebKit website data |
| `$XDG_CACHE_HOME/bitter-ui/` | WebKit cache |

The WebKit directory names match earlier releases, keeping existing website
data available. The browser restricts these directories to their owner
(`0700`) and locks the directories for the lifetime of the application. A
second launch on the same D-Bus session activates the existing window. A
separate session sharing any of these directories receives a profile-in-use
error. Lock files are reusable and should stay in place. For development with
an isolated profile, set separate absolute paths for all three XDG directories
and launch in a separate D-Bus session.

## SQLite migrations and recovery

Each database tracks its schema with SQLite's `user_version`. The first
successful open adopts version 0 databases in a transaction, preserving rows,
IDs and timestamps. The migration rebuilds the history search index once.
SQLite triggers keep the search index up to date after migration.

If a migration fails, SQLite rolls back the transaction for that database.
If startup is blocked by a newer schema, use a browser version that supports
it. For an I/O or space error, correct permissions or free space before trying
again. If the error indicates damage, close any running browser first, then
copy the profile to a safe location, including any SQLite journal, WAL or SHM
files. Inspect the copy before recovering rows or restoring a known-good
backup. The browser preserves the files and reports the affected path.

## Preferences

The toolbar gear, **Ctrl+,**, the command palette and `bitter://settings` open
the same native dialog. Search providers are DuckDuckGo, Brave Search and
Google. The color scheme defaults to System, with Light and Dark overrides for
the browser controls and dialogs. The dialog identifies ad and tracker
blocking and HTTPS-only mode as unavailable.

TOML version 1 accepts missing settings and supplies their defaults. When
saving an older version, the browser retains unknown keys and unrelated
comments. Legacy privacy and session keys stay in the file for compatibility.
A save writes and synchronizes a private temporary file in the same directory
as the original, atomically renames it, then synchronizes the directory.

With unreadable, malformed or unsupported configuration, the browser starts
with defaults and disables editing in Preferences. The dialog shows the file
path and the reason. Correct the file or move it to a backup while the browser
is closed, then restart. If a save fails before the rename, the last effective
settings stay active. If the rename succeeds and directory synchronization
fails, the dialog reports the durability problem.

## Verification

```sh
cargo test --workspace --locked
GTK_A11Y=none GDK_BACKEND=x11 GSK_RENDERER=cairo \
  xvfb-run -a dbus-run-session -- \
  cargo test -p bitter-ui --locked settings_widgets_save_restore_and_report_errors -- --ignored
```

The tests use fresh directories and file-backed SQLite databases. They cover
legacy and fresh profiles, profile contention, SQLite read-only and disk-full
failures, interrupted configuration saves, search encoding, and native
preference changes and errors. The X11 test exercises the actual GTK widgets;
it also runs in CI.

Local validation also opened the native application under X11 with
WebKitGTK 2.54.2. It exercised same-session activation, profile contention
across D-Bus sessions, configuration and database recovery, and saving a
locally served page to history and bookmarks. A freshly built Flatpak ran
with GNOME 51 in a disposable installation. Changing the theme through its
native dialog wrote the configuration in the sandbox's XDG directories.

## Restoring a session

The Startup group in Preferences controls whether the next launch restores
tabs. Session files live in `$XDG_DATA_HOME/bitter-browser/session.json` and
contain tab addresses, titles, order and selection for Main and Second.
Web pages opened in the background and restored tabs create their WebKit view
on first selection. Each workspace can hold 100 tabs. Reopening a recently
closed tab restores its address and position for the current run.

Snapshots retain HTTP(S) addresses and supported internal pages. File
addresses and URLs with embedded credentials are replaced by a local new
tab. Session restoration opens saved addresses and web content again.
Page forms, downloads, per-tab back/forward history and in-page state are
outside the snapshot. Disabling restore takes effect at next launch and
leaves the previous snapshot on disk.

A worker coalesces updates within 250 ms windows, writes snapshots
atomically, and flushes queued changes on normal exit. After an abrupt exit,
the browser loads the last complete snapshot. If that snapshot is unreadable,
the browser opens a fresh session and preserves the existing file. To resume
saving after a corrupt snapshot, close the browser and move the file to a
backup before restarting.
