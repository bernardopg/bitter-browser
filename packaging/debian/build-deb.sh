#!/bin/sh
# Makes the Debian/Ubuntu package from a staged install tree.
#
# Usage: packaging/debian/build-deb.sh VERSION STAGE_DIR OUT_DIR
#
# Runs on Debian 13 (the build base). dpkg-shlibdeps derives the library
# dependencies from the binaries, so the package asks for exactly the
# GTK 4, libadwaita and WebKitGTK 6.0 versions it was linked against.
set -eu

version=${1:?usage: build-deb.sh VERSION STAGE_DIR OUT_DIR}
stage=${2:?}
out=${3:?}
here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
pkg=$work/pkg
arch=$(dpkg --print-architecture)

cp -a "$stage" "$pkg"
# Debian keeps the license in the copyright file, not under /usr/share/licenses.
rm -rf "$pkg/usr/share/licenses"
doc=$pkg/usr/share/doc/bitter-browser
install -Dm644 "$here/copyright" "$doc/copyright"
date=$(date -R -d "@${SOURCE_DATE_EPOCH:-$(date +%s)}")
printf 'bitter-browser (%s-1) unstable; urgency=medium\n\n  * Release %s.\n    https://github.com/bernardopg/bitter-browser/releases/tag/v%s\n\n -- Bitter Browser Contributors <bernardopg@users.noreply.github.com>  %s\n' \
    "$version" "$version" "$version" "$date" | gzip -9n > "$doc/changelog.Debian.gz"
chmod 644 "$doc/changelog.Debian.gz"

mkdir -p "$work/shlibs/debian"
printf 'Source: bitter-browser\n\nPackage: bitter-browser\nArchitecture: any\n' > "$work/shlibs/debian/control"
depends=$(cd "$work/shlibs" && dpkg-shlibdeps -O "$pkg/usr/bin/bitter-ui" "$pkg/usr/bin/bitter-cli" |
    sed -n 's/^shlibs:Depends=//p')
test -n "$depends"

mkdir -p "$pkg/DEBIAN"
size=$(du -sk --exclude=DEBIAN "$pkg" | cut -f1)
cat > "$pkg/DEBIAN/control" <<CONTROL
Package: bitter-browser
Version: $version-1
Architecture: $arch
Maintainer: Bitter Browser Contributors <bernardopg@users.noreply.github.com>
Installed-Size: $size
Depends: $depends
Recommends: gstreamer1.0-plugins-good, gstreamer1.0-plugins-bad, gstreamer1.0-libav, xdg-desktop-portal, xdg-utils
Suggests: hunspell-en-us
Provides: www-browser, gnome-www-browser
Section: web
Priority: optional
Homepage: https://browser.bebitterbebetter.com.br/
Description: focused, native web browser for the Linux desktop
 Bitter Browser is a web browser built with Rust, GTK 4, libadwaita and
 WebKitGTK. It keeps tabs in a vertical sidebar organized in two
 workspaces, restores sessions, and blocks ads and trackers.
CONTROL
install -m755 "$here/postinst" "$here/prerm" "$pkg/DEBIAN/"
(cd "$pkg" && find usr -type f -print0 | sort -z | xargs -0 md5sum) > "$pkg/DEBIAN/md5sums"
chmod 644 "$pkg/DEBIAN/control" "$pkg/DEBIAN/md5sums"

mkdir -p "$out"
deb="$out/bitter-browser_${version}-1_${arch}.deb"
dpkg-deb --root-owner-group -Zxz --build "$pkg" "$deb"
dpkg-deb --info "$deb"
if command -v lintian >/dev/null 2>&1; then
    # Tags that do not apply to a package built outside the Debian archive.
    lintian --fail-on error --suppress-tags \
        no-manual-page,initial-upload-closes-no-bugs,changelog-file-missing-in-native-package,embedded-library,extended-description-line-too-long \
        "$deb"
fi
echo "$deb"
