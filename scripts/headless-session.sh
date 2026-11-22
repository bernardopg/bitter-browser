#!/bin/sh
# Run a command in a throwaway headless desktop session: Xvfb for the display,
# dbus-run-session for the session bus, and a private XDG_RUNTIME_DIR.
#
#   scripts/headless-session.sh [--screen WxHxD] [--] command [args...]
#
# Why the private runtime directory matters: dbus-run-session starts a new
# bus, but the services it activates (xdg-document-portal, xdg-desktop-portal
# and its backends) inherit XDG_RUNTIME_DIR. On a developer machine that is
# /run/user/<uid>, shared with the real desktop session. The test session's
# document portal then takes over /run/user/<uid>/doc and unmounts it when the
# session ends. The desktop's own xdg-document-portal exits with status 21 and
# stays failed, so Flatpak apps lose file access until the next login.
# Inheriting XDG_CURRENT_DESKTOP=Hyprland also made the test bus start
# xdg-desktop-portal-hyprland, which crashes without a compositor and leaves a
# coredump on every run. CI runners have no desktop session, so they never saw
# either problem.
set -eu

screen=""
if [ "${1:-}" = "--screen" ]; then
    [ $# -ge 2 ] || { echo "usage: $0 [--screen WxHxD] [--] command [args...]" >&2; exit 2; }
    screen="$2"
    shift 2
fi
[ "${1:-}" = "--" ] && shift
[ $# -gt 0 ] || { echo "usage: $0 [--screen WxHxD] [--] command [args...]" >&2; exit 2; }

runtime="$(mktemp -d "${TMPDIR:-/tmp}/bitter-runtime.XXXXXX")"
chmod 700 "$runtime"
# shellcheck disable=SC2329  # called by the EXIT trap
cleanup() {
    # A document portal that is still alive keeps a FUSE mount at
    # $runtime/doc. Its files map to real user files, so unmount it first
    # and never let rm cross into another file system.
    if command -v fusermount3 >/dev/null 2>&1; then
        fusermount3 -u -z "$runtime/doc" 2>/dev/null || true
    fi
    rm -rf --one-file-system "$runtime" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT TERM

export XDG_RUNTIME_DIR="$runtime"
# Nothing from the desktop session may leak into the test session.
unset WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS HYPRLAND_INSTANCE_SIGNATURE \
    SWAYSOCK XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP XDG_SESSION_TYPE \
    DESKTOP_SESSION

status=0
if [ -n "$screen" ]; then
    xvfb-run -a -s "-screen 0 $screen" dbus-run-session -- "$@" || status=$?
else
    xvfb-run -a dbus-run-session -- "$@" || status=$?
fi
exit "$status"
