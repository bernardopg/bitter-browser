#!/bin/sh
# Regenerate flatpak/cargo-sources.json from Cargo.lock with a pinned
# flatpak-cargo-generator in a throwaway virtual environment.
set -eu
GENERATOR_COMMIT=9d86d336a39b9af38b86f114b6ce5820fe260f91
GENERATOR_SHA256=0a2db6be87d75910facef28ab46d4d6460802e8419ab850d0caa6a364d26b380
root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsSL -o "$work/flatpak-cargo-generator.py" \
  "https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/$GENERATOR_COMMIT/cargo/flatpak-cargo-generator.py"
echo "$GENERATOR_SHA256  $work/flatpak-cargo-generator.py" | sha256sum -c --quiet
python3 -m venv "$work/venv"
"$work/venv/bin/pip" install --quiet aiohttp tomlkit
"$work/venv/bin/python" -I "$work/flatpak-cargo-generator.py" \
  "$root/Cargo.lock" -o "$root/flatpak/cargo-sources.json"
python3 "$root/scripts/check-flatpak-sources.py"
