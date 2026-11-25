#!/bin/sh
# Signs the native Arch package and builds the signed pacman repository
# published with each release. Run in the archlinux container after
# build-arch.sh.
#
# Usage: packaging/arch/make-repo.sh PKG_DIR OUT_DIR
#
# Needs PACMAN_GPG_PRIVATE_KEY (armored secret key) and PACMAN_GPG_FINGERPRINT.
# pacman reads the repository through
#   Server = https://github.com/bernardopg/bitter-browser/releases/latest/download
# so every file is a flat release asset: no symbolic links.
set -eu

pkg_dir=$(realpath "${1:?usage: make-repo.sh PKG_DIR OUT_DIR}")
out=$(realpath -m "${2:?}")
: "${PACMAN_GPG_PRIVATE_KEY:?}" "${PACMAN_GPG_FINGERPRINT:?}"

GNUPGHOME=$(mktemp -d)
export GNUPGHOME
trap 'rm -rf "$GNUPGHOME"' EXIT
chmod 700 "$GNUPGHOME"
printf '%s\n' "$PACMAN_GPG_PRIVATE_KEY" | gpg --batch --import
gpg --batch --list-secret-keys "$PACMAN_GPG_FINGERPRINT" >/dev/null

work=$(mktemp -d)
# Only the package built from source goes into the repository; the -bin
# package is for the AUR.
cp "$pkg_dir"/bitter-browser-[0-9]*.pkg.tar.zst "$work/"
cd "$work"
for package in ./*.pkg.tar.zst; do
    gpg --batch --yes --local-user "$PACMAN_GPG_FINGERPRINT" --detach-sign --no-armor "$package"
    gpg --batch --verify "$package.sig" "$package"
done
repo-add --sign --key "$PACMAN_GPG_FINGERPRINT" bitter-browser.db.tar.gz ./*.pkg.tar.zst

mkdir -p "$out"
for file in *.pkg.tar.zst *.pkg.tar.zst.sig; do cp "$file" "$out/"; done
# pacman downloads <repo>.db and <repo>.files; repo-add makes them symbolic links.
for file in bitter-browser.db bitter-browser.db.sig bitter-browser.files bitter-browser.files.sig; do
    cp -L "$file" "$out/$file"
done
ls -la "$out"
