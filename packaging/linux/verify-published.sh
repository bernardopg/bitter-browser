#!/bin/sh
# Installs a published release the way users do and checks the result.
# Run as root in a container of the target distribution.
#
# Usage: packaging/linux/verify-published.sh deb|rpm|pacman|tarball|pacman-repo VERSION
set -eu

method=${1:?usage: verify-published.sh METHOD VERSION}
version=${2:?}
repo=bernardopg/bitter-browser
base=https://github.com/$repo/releases/download/v$version
here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

case $method in
    deb) apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y curl ca-certificates ;;
    rpm) dnf install -y curl ;;
    pacman|pacman-repo) pacman -Syu --noconfirm --needed curl ;;
    tarball) zypper --non-interactive install curl tar xz libwebkitgtk-6_0-4 libadwaita-1-0 glib2-tools ;;
esac

if [ "$method" = pacman-repo ]; then
    # The documented setup: trust the signing key, then add the repository.
    pacman-key --init
    curl -fsSL "$base/bitter-browser-packages.asc" -o /tmp/key.asc
    fingerprint=$(gpg --show-keys --with-colons /tmp/key.asc | awk -F: '/^fpr/ {print $10; exit}')
    test "$fingerprint" = "$(gpg --show-keys --with-colons "$here/../arch/bitter-browser-packages.asc" | awk -F: '/^fpr/ {print $10; exit}')"
    pacman-key --add /tmp/key.asc
    pacman-key --lsign-key "$fingerprint"
    printf '\n[bitter-browser]\nServer = https://github.com/%s/releases/latest/download\n' "$repo" >> /etc/pacman.conf
    pacman -Sy --noconfirm bitter-browser
    bin=/usr/bin
else
    curl -fsSL "$base/install.sh" -o /tmp/install.sh
    sh /tmp/install.sh --method "$method" --version "$version"
    case $method in tarball) bin=$HOME/.local/bin ;; *) bin=/usr/bin ;; esac
fi

test "$("$bin/bitter-ui" --version)" = "Bitter Browser $version"
test "$("$bin/bitter-cli" --version)" = "Bitter Browser $version"
echo "Published release $version installs with $method"
