## Install

| System | Command |
| --- | --- |
| Any distribution | `curl -fsSL https://browser.bebitterbebetter.com.br/install.sh \| sh` |
| Arch Linux (AUR) | `yay -S bitter-browser` or `yay -S bitter-browser-bin` |
| Arch Linux (pacman) | [Add the signed repository](https://github.com/bernardopg/bitter-browser/blob/main/docs/distribution.md#arch-linux-pacman-repository), then `sudo pacman -S bitter-browser` |
| Debian 13, Ubuntu 25.10+, Linux Mint 23 | `sudo apt install ./bitter-browser_@VERSION@-1_amd64.deb` |
| Fedora 43+ | `sudo dnf install ./bitter-browser-@VERSION@-1.fc44.x86_64.rpm` |
| Flatpak | `flatpak install --user ./bitter-browser-@VERSION@-x86_64.flatpak` |
| Other (glibc 2.41+) | `tar -xJf bitter-browser-@VERSION@-linux-x86_64.tar.xz && ./bitter-browser-@VERSION@-linux-x86_64/install.sh` |

Check a download: `sha256sum -c bitter-browser-@VERSION@-sha256.txt --ignore-missing` and
`gh attestation verify <file> --repo bernardopg/bitter-browser`.
Every distribution and desktop: [docs/distribution.md](https://github.com/bernardopg/bitter-browser/blob/main/docs/distribution.md).
