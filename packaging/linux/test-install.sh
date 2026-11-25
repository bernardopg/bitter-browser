#!/bin/sh
# Installs a built package on the running distribution, checks the installed
# files and the version, then removes it again. Run as root in a container.
#
# Usage: packaging/linux/test-install.sh deb|rpm|pacman|tarball VERSION PKG_DIR
set -eu

format=${1:?usage: test-install.sh FORMAT VERSION PKG_DIR}
version=${2:?}
pkgs=$(realpath "${3:?}")
app_id=br.com.bebitterbebetter.browser.Bitter
prefix=/usr

case $format in
    deb)
        export DEBIAN_FRONTEND=noninteractive
        # Development images (Linux Mint) can list a retired repository; the
        # distribution's own archives are enough for the dependencies.
        apt-get update || echo "Some repositories could not be updated; continuing"
        apt-get install -y --no-install-recommends "$pkgs/bitter-browser_${version}-1_amd64.deb"
        update-alternatives --query x-www-browser | grep -Fx 'Value: /usr/bin/bitter-ui'
        ;;
    rpm)
        rpm=$(find "$pkgs" -name "bitter-browser-$version-1.*.x86_64.rpm" | head -n 1)
        test -n "$rpm"
        dnf install -y "$rpm"
        ;;
    pacman)
        pacman -Syu --noconfirm
        pacman -U --noconfirm "$pkgs/pkg/bitter-browser-$version-1-x86_64.pkg.tar.zst"
        ;;
    tarball)
        zypper --non-interactive install libwebkitgtk-6_0-4 libadwaita-1-0 tar xz findutils glib2-tools
        tar -xJf "$pkgs/bitter-browser-$version-linux-x86_64.tar.xz" -C /tmp
        "/tmp/bitter-browser-$version-linux-x86_64/install.sh"
        prefix=/usr/local
        ;;
    *) echo "unknown format $format" >&2; exit 2 ;;
esac

test "$("$prefix/bin/bitter-ui" --version)" = "Bitter Browser $version"
test "$("$prefix/bin/bitter-cli" --version)" = "Bitter Browser $version"
for file in \
    "share/applications/$app_id.desktop" \
    "share/metainfo/$app_id.metainfo.xml" \
    "share/icons/hicolor/scalable/apps/$app_id.svg" \
    "share/icons/hicolor/128x128/apps/$app_id.png" \
    "share/glib-2.0/schemas/$app_id.gschema.xml" \
    "share/xfce4/helpers/bitter-browser.desktop"
do
    test -f "$prefix/$file" || { echo "not installed: $prefix/$file" >&2; exit 1; }
done
grep -q '^MimeType=.*x-scheme-handler/https;' "$prefix/share/applications/$app_id.desktop"
echo "Bitter Browser $version installed correctly as $format"

case $format in
    deb) apt-get remove -y bitter-browser ;;
    rpm) dnf remove -y bitter-browser ;;
    pacman) pacman -R --noconfirm bitter-browser ;;
    tarball) "/tmp/bitter-browser-$version-linux-x86_64/install.sh" --uninstall ;;
esac
if test -e "$prefix/bin/bitter-ui"; then echo "bitter-ui is still installed" >&2; exit 1; fi
echo "Removed cleanly"
