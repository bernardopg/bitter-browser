# Maintainer: Bitter Browser Contributors <https://github.com/bernardopg/bitter-browser>
# Development package: builds the latest commit of the main branch.
pkgname=bitter-browser-git
pkgver=0.2.1.r0.g0000000
pkgrel=1
pkgdesc='Focused, native web browser for the Linux desktop'
arch=('x86_64')
url='https://github.com/bernardopg/bitter-browser'
license=('MPL-2.0')
depends=('dconf' 'glib2' 'glibc' 'gtk4' 'hicolor-icon-theme' 'libadwaita' 'libgcc' 'libsoup3' 'pango' 'webkitgtk-6.0')
makedepends=('cargo' 'git' 'meson')
# The bundled SQLite is C; makepkg LTO turns it into GCC bitcode that the Rust linker cannot read.
options=('!lto')
provides=('bitter-browser')
conflicts=('bitter-browser')
source=('git+https://github.com/bernardopg/bitter-browser.git')
sha256sums=('SKIP')

pkgver() {
    cd bitter-browser
    git describe --long --tags --abbrev=7 | sed 's/^v//;s/\([^-]*-g\)/r\1/;s/-/./g'
}

prepare() {
    cd bitter-browser
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc --print host-tuple)"
}

build() {
    cd bitter-browser
    export RUSTUP_TOOLCHAIN=stable CARGO_NET_OFFLINE=true
    arch-meson . build
    meson compile -C build
}

check() {
    cd bitter-browser
    export RUSTUP_TOOLCHAIN=stable CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=build/cargo-target
    cargo test --frozen --release --workspace
}

package() {
    cd bitter-browser
    meson install -C build --destdir "$pkgdir"
}
