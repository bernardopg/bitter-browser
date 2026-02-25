pkgname=bitter-browser-git
pkgver=0.1.0
pkgrel=1
pkgdesc="The browser that bites back — fast, sharp, and precise."
arch=('x86_64')
url="https://github.com/bernardopg/bitter-browser"
license=('MPL2')
depends=('gtk4' 'libadwaita' 'webkitgtk-6.0' 'sqlite' 'openssl')
makedepends=('cargo' 'meson' 'ninja' 'git')
provides=('bitter-browser')
conflicts=('bitter-browser')
source=("git+https://github.com/bernardopg/bitter-browser.git")
sha256sums=('SKIP')

pkgver() {
    cd "$srcdir/bitter-browser"
    git describe --long --tags | sed 's/\([^-]*-g\)/r\1/;s/-/./g'
}

build() {
    cd "$srcdir/bitter-browser"
    arch-meson . build
    meson compile -C build
}

package() {
    cd "$srcdir/bitter-browser"
    meson install -C build --destdir "$pkgdir"
}
