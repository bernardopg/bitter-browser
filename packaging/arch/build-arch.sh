#!/bin/sh
# Builds and checks the Arch Linux packages and prepares the AUR recipes.
# Run as root in an archlinux:base-devel container.
#
# Usage: packaging/arch/build-arch.sh VERSION SOURCE_ARCHIVE BINARY_ARCHIVE OUT_DIR
#
# OUT_DIR receives:
#   aur/<pkgname>/{PKGBUILD,.SRCINFO}  recipes with the real checksums, for the AUR
#   pkg/*.pkg.tar.zst                  the packages built from those recipes
# The archives are the release assets; makepkg uses the local copies, so the
# recipes are built and checked before anything is published.
set -eu

version=${1:?usage: build-arch.sh VERSION SOURCE_ARCHIVE BINARY_ARCHIVE OUT_DIR}
source_archive=$(realpath "${2:?}")
binary_archive=$(realpath "${3:?}")
out=$(realpath -m "${4:?}")
root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)

pacman -Syu --noconfirm --needed base-devel git namcap sudo
id builder >/dev/null 2>&1 || useradd --create-home builder
printf 'builder ALL=(root) NOPASSWD: /usr/bin/pacman\n' > /etc/sudoers.d/builder
chmod 0440 /etc/sudoers.d/builder
mkdir -p "$out/aur" "$out/pkg"

# prepare NAME RECIPE ARCHIVE CHECKSUM_FIELD: copies a recipe, pins the version
# and the archive's checksum, and puts the archive where makepkg finds it.
prepare() {
    name=$1 recipe=$2 archive=$3 field=$4
    dir=/home/builder/$name
    rm -rf "$dir"
    mkdir -p "$dir"
    cp "$recipe" "$dir/PKGBUILD"
    sed -i "s/^pkgver=.*/pkgver=$version/; s/^pkgrel=.*/pkgrel=1/" "$dir/PKGBUILD"
    if [ -n "$archive" ]; then
        sum=$(sha256sum "$archive" | cut -d' ' -f1)
        sed -i "s/^$field=('SKIP')\$/$field=('$sum')/" "$dir/PKGBUILD"
        grep -q "^$field=('$sum')\$" "$dir/PKGBUILD"
        cp "$archive" "$dir/"
    fi
    chown -R builder: "$dir"
}

build() {
    dir=/home/builder/$1
    shift
    (cd "$dir" && runuser -u builder -- makepkg --syncdeps --noconfirm --cleanbuild "$@")
    namcap "$dir/PKGBUILD"
    for package in "$dir"/*.pkg.tar.zst; do
        # makepkg's separate debug package is not published.
        case $package in *-debug-*) continue ;; esac
        namcap "$package"
        cp "$package" "$out/pkg/"
    done
}

publish_recipe() {
    dir=/home/builder/$1
    mkdir -p "$out/aur/$1"
    cp "$dir/PKGBUILD" "$out/aur/$1/"
    (cd "$dir" && runuser -u builder -- makepkg --printsrcinfo) > "$out/aur/$1/.SRCINFO"
}

prepare bitter-browser "$root/packaging/arch/PKGBUILD" "$source_archive" sha256sums
build bitter-browser
publish_recipe bitter-browser

prepare bitter-browser-bin "$root/packaging/arch-bin/PKGBUILD" "$binary_archive" sha256sums_x86_64
build bitter-browser-bin
publish_recipe bitter-browser-bin

# The -git recipe builds whatever main holds when a user installs it; only
# its metadata is checked here.
prepare bitter-browser-git "$root/PKGBUILD" "" ""
sed -i "s/^pkgver=.*/pkgver=$version.r0.g${GIT_COMMIT:-0000000}/" /home/builder/bitter-browser-git/PKGBUILD
namcap /home/builder/bitter-browser-git/PKGBUILD
publish_recipe bitter-browser-git

# Smoke test: the native package installs and starts on Arch Linux.
pacman -U --noconfirm "$out/pkg/bitter-browser-$version-1-x86_64.pkg.tar.zst"
bitter-ui --version
bitter-cli --version
pacman -R --noconfirm bitter-browser
pacman -U --noconfirm "$out/pkg/bitter-browser-bin-$version-1-x86_64.pkg.tar.zst"
bitter-ui --version
pacman -R --noconfirm bitter-browser-bin
ls -la "$out/pkg" "$out/aur"/*
