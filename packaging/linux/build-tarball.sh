#!/bin/sh
# Makes the portable binary archive (.tar.xz) from a staged install tree.
#
# Usage: packaging/linux/build-tarball.sh VERSION STAGE_DIR OUT_DIR
#
# The archive holds bin/ and share/ in prefix layout plus install.sh, which
# installs into /usr/local (root) or ~/.local (user). It needs GTK 4.14,
# libadwaita 1.6 and WebKitGTK 6.0 (2.44) from the distribution.
set -eu

version=${1:?usage: build-tarball.sh VERSION STAGE_DIR OUT_DIR}
stage=${2:?}
out=${3:?}
here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH='' cd -- "$here/../.." && pwd)
machine=$(uname -m)
name=bitter-browser-$version-linux-$machine
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/$name"
cp -a "$stage/usr/bin" "$stage/usr/share" "$work/$name/"
install -m755 "$here/tarball-install.sh" "$work/$name/install.sh"
install -m644 "$root/LICENSE" "$work/$name/LICENSE"
install -m644 "$here/tarball-README.txt" "$work/$name/README.txt"

mkdir -p "$out"
# Reproducible metadata: fixed owner, sorted names, source date as mtime.
tar --sort=name --owner=0 --group=0 --numeric-owner \
    --mtime="@${SOURCE_DATE_EPOCH:-0}" -C "$work" -cf - "$name" |
    xz -T0 -9e > "$out/$name.tar.xz"
tar -tJf "$out/$name.tar.xz" | head -n 5
echo "$out/$name.tar.xz"
