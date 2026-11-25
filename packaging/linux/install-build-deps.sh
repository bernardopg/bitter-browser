#!/bin/sh
# Installs the packages and Rust toolchain needed to build Bitter Browser on
# Debian 13 or newer (and Ubuntu 25.10 or newer). Run as root.
set -eu
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl dpkg-dev file git meson pkg-config \
    xz-utils desktop-file-utils appstream lintian fakeroot binutils \
    libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev libsoup-3.0-dev \
    libglib2.0-bin
if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs |
        sh -s -- -y --profile minimal --default-toolchain stable
fi
