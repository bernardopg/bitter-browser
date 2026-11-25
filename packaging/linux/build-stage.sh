#!/bin/sh
# Builds Bitter Browser with Meson and installs it into a staging directory
# with the /usr layout. The portable archive and the .deb are made from
# this tree.
#
# Usage: packaging/linux/build-stage.sh STAGE_DIR
#
# Run it from the source tree on the oldest supported base (Debian 13), so
# the binaries need no newer glibc than every supported distribution has.
# Install the build dependencies first with packaging/linux/install-build-deps.sh.
set -eu

stage=${1:?usage: build-stage.sh STAGE_DIR}
root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
build=${BUILD_DIR:-$root/build-dist}

rm -rf "$build" "$stage"
meson setup "$build" "$root" --prefix=/usr --buildtype=release
meson compile -C "$build"
DESTDIR=$(realpath -m "$stage") meson install -C "$build" --no-rebuild
strip --strip-unneeded "$stage/usr/bin/bitter-ui" "$stage/usr/bin/bitter-cli"

# The stage must hold the whole documented install set.
for path in \
    usr/bin/bitter-ui \
    usr/bin/bitter-cli \
    usr/share/applications/br.com.bebitterbebetter.browser.Bitter.desktop \
    usr/share/metainfo/br.com.bebitterbebetter.browser.Bitter.metainfo.xml \
    usr/share/glib-2.0/schemas/br.com.bebitterbebetter.browser.Bitter.gschema.xml \
    usr/share/icons/hicolor/scalable/apps/br.com.bebitterbebetter.browser.Bitter.svg \
    usr/share/icons/hicolor/256x256/apps/br.com.bebitterbebetter.browser.Bitter.png \
    usr/share/xfce4/helpers/bitter-browser.desktop
do
    test -e "$stage/$path" || { echo "missing from the stage: $path" >&2; exit 1; }
done
"$stage/usr/bin/bitter-ui" --version
"$stage/usr/bin/bitter-cli" --version
