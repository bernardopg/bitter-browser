#!/bin/sh
# Installs Bitter Browser from this unpacked archive.
#
#   ./install.sh                 install (/usr/local as root, ~/.local otherwise)
#   ./install.sh --prefix DIR    install below DIR
#   ./install.sh --uninstall     remove a previous install from the same prefix
#
# The browser uses GTK 4, libadwaita and WebKitGTK 6.0 from your
# distribution; the script stops and names them when one is missing.
set -eu

here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
prefix=
uninstall=0
force=0
while [ $# -gt 0 ]; do
    case $1 in
        --prefix) prefix=${2:?--prefix needs a directory}; shift 2 ;;
        --prefix=*) prefix=${1#--prefix=}; shift ;;
        --uninstall) uninstall=1; shift ;;
        --force) force=1; shift ;;
        -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
        *) echo "install.sh: unknown option $1" >&2; exit 2 ;;
    esac
done
if [ -z "$prefix" ]; then
    if [ "$(id -u)" = 0 ]; then prefix=/usr/local; else prefix=$HOME/.local; fi
fi
manifest=$prefix/share/bitter-browser/installed-files

refresh() {
    glib-compile-schemas "$prefix/share/glib-2.0/schemas" 2>/dev/null || true
    update-desktop-database -q "$prefix/share/applications" 2>/dev/null || true
    gtk-update-icon-cache -q -f -t "$prefix/share/icons/hicolor" 2>/dev/null || true
}

if [ "$uninstall" = 1 ]; then
    [ -f "$manifest" ] || { echo "No Bitter Browser install found in $prefix" >&2; exit 1; }
    while IFS= read -r file; do rm -f "$prefix/$file"; done < "$manifest"
    rm -f "$manifest"
    rmdir "$prefix/share/bitter-browser" 2>/dev/null || true
    refresh
    echo "Removed Bitter Browser from $prefix"
    exit 0
fi

missing=$(ldd "$here/bin/bitter-ui" 2>/dev/null | sed -n 's/^[[:space:]]*\([^ ]*\) => not found$/\1/p')
if [ -n "$missing" ] && [ "$force" = 0 ]; then
    echo "These libraries are missing:" >&2
    echo "$missing" | sed 's/^/  /' >&2
    cat >&2 <<'HELP'
Install them first:
  Debian 13, Ubuntu 25.10+:  sudo apt install libwebkitgtk-6.0-4 libadwaita-1-0 libgtk-4-1
  Fedora:                    sudo dnf install webkitgtk6.0 libadwaita gtk4
  Arch Linux:                sudo pacman -S webkitgtk-6.0 libadwaita gtk4
  openSUSE:                  sudo zypper install libwebkitgtk-6_0-4 libadwaita-1-0 gtk4
Or use the Flatpak, which brings its own libraries.
HELP
    exit 1
fi

mkdir -p "$prefix/share/bitter-browser"
: > "$manifest.new"
(cd "$here" && find bin share -type f | sort) | while IFS= read -r file; do
    case $file in bin/*) mode=755 ;; *) mode=644 ;; esac
    install -D -m "$mode" "$here/$file" "$prefix/$file"
    echo "$file" >> "$manifest.new"
done
mv "$manifest.new" "$manifest"
# Launchers call the installed binary even when $prefix/bin is not on PATH.
sed -i "s|^Exec=bitter-ui|Exec=$prefix/bin/bitter-ui|" \
    "$prefix/share/applications/br.com.bebitterbebetter.browser.Bitter.desktop"
sed -i "s|^X-XFCE-Commands=bitter-ui|X-XFCE-Commands=$prefix/bin/bitter-ui|; s|^X-XFCE-CommandsWithParameter=bitter-ui|X-XFCE-CommandsWithParameter=$prefix/bin/bitter-ui|" \
    "$prefix/share/xfce4/helpers/bitter-browser.desktop"
refresh

echo "Installed $("$prefix/bin/bitter-ui" --version 2>/dev/null || echo "Bitter Browser") in $prefix"
case :$PATH: in
    *:"$prefix/bin":*) ;;
    *) echo "Note: $prefix/bin is not on your PATH; the menu entry works anyway." ;;
esac
echo "Make it the default browser in Preferences > General, or run:"
echo "  xdg-settings set default-web-browser br.com.bebitterbebetter.browser.Bitter.desktop"
