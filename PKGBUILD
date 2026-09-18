# Maintainer: Flavien Bazin <bazinfla@users.noreply.github.com>
pkgname=neuradex
pkgver=0.1.8
pkgrel=1
pkgdesc="Native Linux desktop control center, model hub and inference studio for local AI (Ollama)"
arch=('x86_64')
url="https://github.com/BazinFla/NeuraDex"
license=('GPL-3.0-or-later')
depends=('gtk4' 'libadwaita' 'openssl')
makedepends=('cargo' 'pkgconf')
provides=('neuradex')
conflicts=('neuradex-bin')

pkgver() {
    if [ -f "Cargo.toml" ]; then
        grep -m1 '^version' Cargo.toml | cut -d'"' -f2
    fi
}

build() {
    cargo build --release --locked
}

package() {
    install -Dm755 "target/release/neuradex" "${pkgdir}/usr/bin/neuradex"
    install -Dm644 "data/io.github.bazinfla.NeuraDex.desktop" "${pkgdir}/usr/share/applications/io.github.bazinfla.NeuraDex.desktop"
    install -Dm644 "data/icons/io.github.bazinfla.NeuraDex.svg" "${pkgdir}/usr/share/icons/hicolor/scalable/apps/io.github.bazinfla.NeuraDex.svg"
}
