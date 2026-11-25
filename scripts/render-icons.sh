#!/bin/sh
# Renders the PNG application icons from the scalable SVG.
# Run after changing the SVG and commit the result. Needs rsvg-convert (librsvg).
set -eu
id=br.com.bebitterbebetter.browser.Bitter
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
icons="$root/crates/bitter-ui/data/icons/hicolor"
svg="$icons/scalable/apps/$id.svg"
for size in 16 24 32 48 64 128 256 512; do
    mkdir -p "$icons/${size}x${size}/apps"
    rsvg-convert --width "$size" --height "$size" --keep-aspect-ratio \
        --output "$icons/${size}x${size}/apps/$id.png" "$svg"
done
echo "Rendered PNG icons from $svg"
