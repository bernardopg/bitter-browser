#!/bin/sh
# Assembles the website (site/ plus the brand assets) into OUT_DIR and makes
# the media kit archive and the brand book PDF.
#
# Usage: scripts/build-site.sh OUT_DIR
# Needs zip and Google Chrome or Chromium (for the PDF).
set -eu

out=${1:?usage: build-site.sh OUT_DIR}
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
rm -rf "$out"
mkdir -p "$out"
cp -a "$root/site/." "$out/"
# site/assets/screens comes with site/ (branding/screenshots/compose.py).
cp -a "$root/branding/logo" "$root/branding/icon" "$root/branding/social" "$root/branding/tokens" "$out/assets/"
cp "$root/branding/icon/favicon.ico" "$out/favicon.ico"

# Media kit: every asset plus the brand book source.
kit=$(mktemp -d)
trap 'rm -rf "$kit"; [ -n "${server:-}" ] && kill "$server" 2>/dev/null || true' EXIT
mkdir -p "$kit/bitter-browser-media-kit"
cp -a "$root/branding/logo" "$root/branding/icon" "$root/branding/social" "$root/branding/tokens" \
    "$root/branding/README.md" "$kit/bitter-browser-media-kit/"
mkdir -p "$kit/bitter-browser-media-kit/screenshots"
cp "$root"/docs/assets/screenshots/*.png "$kit/bitter-browser-media-kit/screenshots/"
cp "$root"/site/assets/screens/*-2400.webp "$kit/bitter-browser-media-kit/screenshots/"
(cd "$kit" && zip -qr -X "$out/brand/bitter-browser-media-kit.zip" bitter-browser-media-kit)

# Brand book PDF, printed from the brand page.
chrome=$(command -v google-chrome-stable || command -v google-chrome || command -v chromium || true)
if [ -n "$chrome" ]; then
    port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
    python3 -m http.server "$port" --bind 127.0.0.1 --directory "$out" >/dev/null 2>&1 &
    server=$!
    sleep 1
    "$chrome" --headless=new --disable-gpu --no-pdf-header-footer --no-sandbox \
        --print-to-pdf="$out/brand/bitter-browser-brand-book.pdf" \
        "http://127.0.0.1:$port/brand/" 2>/dev/null
    kill "$server"
    server=
    test -s "$out/brand/bitter-browser-brand-book.pdf"
else
    echo "No Chrome or Chromium: skipping the brand book PDF" >&2
fi
du -sh "$out"
