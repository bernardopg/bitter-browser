# Distribution and desktop integration

Every release is built, tested and published by the *Release* workflow
(`.github/workflows/release.yml`), which calls the *Packages* workflow
(`.github/workflows/packages.yml`). This page lists the install channels, the
supported systems, how the browser integrates with each desktop, and what a
maintainer needs to keep the pipeline running.

Application ID: `br.com.bebitterbebetter.browser.Bitter`
(website: <https://browser.bebitterbebetter.com.br/>).

## Channels

| Channel | Package | Built on | Updates |
| --- | --- | --- | --- |
| One-line installer | picks the native package below | — | run it again |
| AUR | `bitter-browser` (from source), `bitter-browser-bin` (prebuilt), `bitter-browser-git` (main) | Arch Linux | AUR helper (`yay`, `paru`) |
| pacman repository | `bitter-browser`, signed | Arch Linux | `pacman -Syu` |
| `.deb` | `bitter-browser_X.Y.Z-1_amd64.deb` | Debian 13 | download the new release |
| `.rpm` | `bitter-browser-X.Y.Z-1.fc44.x86_64.rpm` and `.src.rpm` | Fedora 44 | download the new release |
| Flatpak bundle | `bitter-browser-X.Y.Z-x86_64.flatpak` | GNOME 51 runtime | download the new release |
| Portable archive | `bitter-browser-X.Y.Z-linux-x86_64.tar.xz` | Debian 13 | download the new release |
| Source | `bitter-browser-X.Y.Z.tar.gz` | — | — |

Every release asset has a SHA-256 checksum in `bitter-browser-X.Y.Z-sha256.txt`
and a GitHub build provenance attestation. The release also carries an SPDX
SBOM, the release `PKGBUILD`, `install.sh` and the pacman signing key.

## Supported systems

The native packages use the distribution's GTK 4 (4.14 or newer), libadwaita
(1.6 or newer) and WebKitGTK 6.0 (2.44 or newer), and need glibc 2.41 or newer
for the prebuilt binaries. Only x86-64 is built today.

| Distribution | Recommended channel | Notes |
| --- | --- | --- |
| Arch Linux, EndeavourOS, Manjaro, CachyOS | AUR or pacman repository | Tested on Arch in CI on every release. |
| Debian 13 (trixie) and newer | `.deb` | Tested in CI. |
| Ubuntu 25.10, 26.04 LTS and newer | `.deb` | Tested on 26.04 in CI, including a full browser smoke test. |
| Linux Mint 23, Pop!_OS and other Ubuntu 26.04 derivatives | `.deb` | Mint 23 install is tested in CI. |
| Fedora 43, 44 and newer | `.rpm` | Tested on 44 and Rawhide in CI. |
| openSUSE Tumbleweed | portable archive or Flatpak | Tested in CI. |
| Ubuntu 24.04, Linux Mint 22, Debian 12 | Flatpak | Their libadwaita and glibc are too old for the native packages. |
| Anything else with Flatpak | Flatpak | Brings its own libraries. |

## Install

### One-line installer

```sh
curl -fsSL https://browser.bebitterbebetter.com.br/install.sh | sh
```

The installer is `scripts/install.sh`; read it before you pipe it to `sh`. It
detects Debian/Ubuntu, Fedora and Arch and installs the native package; on
other systems it installs the portable archive into `~/.local`. It downloads
from the GitHub release and checks each file against the release checksum
file before it installs anything.

```sh
sh install.sh --method flatpak          # deb, rpm, pacman, tarball or flatpak
sh install.sh --version 0.6.0           # a specific release
sh install.sh --method tarball --uninstall
sh install.sh --dry-run                 # print the commands only
```

### Arch Linux: AUR

```sh
yay -S bitter-browser        # builds the release from source
yay -S bitter-browser-bin    # the prebuilt portable binary, no Rust toolchain needed
yay -S bitter-browser-git    # the latest commit on main
```

`paru`, `pikaur` and `makepkg -si` work the same way. The three packages
conflict with each other.

### Arch Linux: pacman repository

The release workflow signs the package and the repository database with the
*Bitter Browser Packages* key
(`A22A FBDC DAA1 4B69 5822  8038 CAEA 8702 9C2D 9818`, also in
`packaging/arch/bitter-browser-packages.asc`).

```sh
curl -fsSL https://github.com/bernardopg/bitter-browser/releases/latest/download/bitter-browser-packages.asc \
  | sudo pacman-key --add -
sudo pacman-key --lsign-key A22AFBDCDAA14B6958228038CAEA87029C2D9818
printf '\n[bitter-browser]\nServer = https://github.com/bernardopg/bitter-browser/releases/latest/download\n' \
  | sudo tee -a /etc/pacman.conf
sudo pacman -Syu bitter-browser
```

Compare the fingerprint `pacman-key` prints with the one above before you sign
the key.

### Debian, Ubuntu and Linux Mint

```sh
sudo apt install ./bitter-browser_0.6.0-1_amd64.deb
```

APT installs the library dependencies. The package registers the browser for
the `x-www-browser` and `gnome-www-browser` alternatives with priority 40, so
it does not replace a browser you already chose:
`sudo update-alternatives --config x-www-browser` selects it.

### Fedora

```sh
sudo dnf install ./bitter-browser-0.6.0-1.fc44.x86_64.rpm
```

`packaging/rpm/bitter-browser.spec` builds the same package from source with
`rpmbuild` or in COPR (enable network access for the build: Cargo downloads
the crates pinned in `Cargo.lock`).

### Flatpak

```sh
flatpak install --user ./bitter-browser-0.6.0-x86_64.flatpak
flatpak run br.com.bebitterbebetter.browser.Bitter
```

The bundle pulls the GNOME runtime from Flathub. Bitter Browser is not on
Flathub yet; see [Flathub](#flathub).

### Portable archive

```sh
tar -xJf bitter-browser-0.6.0-linux-x86_64.tar.xz
./bitter-browser-0.6.0-linux-x86_64/install.sh          # ~/.local
sudo ./bitter-browser-0.6.0-linux-x86_64/install.sh     # /usr/local
```

`install.sh` stops and names the packages to install when a library is
missing. `--uninstall` removes the files it installed.

### Verify a download

```sh
sha256sum -c --ignore-missing bitter-browser-0.6.0-sha256.txt
gh attestation verify bitter-browser_0.6.0-1_amd64.deb --repo bernardopg/bitter-browser
```

## Desktop integration

Every native package installs the same files (`meson.build`):

| File | Purpose |
| --- | --- |
| `/usr/share/applications/br.com.bebitterbebetter.browser.Bitter.desktop` | Menu entry, `WebBrowser` category, `http`, `https` and HTML MIME types, `StartupWMClass` for docks |
| `/usr/share/icons/hicolor/{16..512}/apps/…png`, `scalable/apps/…svg` | Icons for menus, docks and window switchers on every desktop |
| `/usr/share/metainfo/br.com.bebitterbebetter.browser.Bitter.metainfo.xml` | Listing in GNOME Software and KDE Discover |
| `/usr/share/glib-2.0/schemas/…gschema.xml` | Window size and maximized state |
| `/usr/share/xfce4/helpers/bitter-browser.desktop` | Xfce *Default Applications* web browser entry |
| `/usr/share/licenses/bitter-browser/LICENSE` | License (Debian: `/usr/share/doc/bitter-browser/copyright`) |

Package triggers refresh the desktop database, icon cache and GSettings
schemas after install and removal.

### Make Bitter Browser the default browser

In the browser: **Preferences > General > System > Default browser > Make
Default**. It registers the browser for `http`, `https`, `text/html` and
`application/xhtml+xml` in `~/.config/mimeapps.list` and then runs
`xdg-settings`, which updates the settings that some desktops keep apart.
In the Flatpak the row points to the system settings instead, because the
sandbox cannot change them.

From a terminal on any desktop:

```sh
xdg-settings set default-web-browser br.com.bebitterbebetter.browser.Bitter.desktop
xdg-settings get default-web-browser
```

| Desktop | Where to choose it | Stored in |
| --- | --- | --- |
| GNOME | Settings > Apps > Default Apps > Web | `mimeapps.list` |
| KDE Plasma | System Settings > Default Applications > Web browser | `kdeglobals` and `mimeapps.list` |
| Xfce | Settings > Default Applications > Internet > Web Browser | exo helpers and `mimeapps.list` |
| Cinnamon | System Settings > Preferred Applications > Web | `mimeapps.list` |
| MATE | Preferred Applications > Internet > Web Browser | `mimeapps.list` |
| Budgie, Hyprland, Sway and others | `xdg-settings` (above) or `handlr` | `mimeapps.list` |

On Debian-based systems, `sudo update-alternatives --config x-www-browser`
also chooses the browser for programs that call `x-www-browser` or
`sensible-browser`.

## Upgrading from com.bitterbrowser.Browser

Before 0.6.0 the application ID was `com.bitterbrowser.Browser`. The domain
`bitterbrowser.com` belongs to another product, so the ID moved to a domain
the project controls.

- Native packages: the browser profile (`~/.config/bitter-browser`,
  `~/.local/share/bitter-browser`) is unchanged. The saved window size resets
  once. Choose the browser as default again if it was the default.
- Flatpak: the profile moves with the ID. Remove the old app and copy its
  data before the first start of the new one:

  ```sh
  flatpak uninstall --user com.bitterbrowser.Browser
  mkdir -p ~/.var/app/br.com.bebitterbebetter.browser.Bitter
  cp -a ~/.var/app/com.bitterbrowser.Browser/. ~/.var/app/br.com.bebitterbebetter.browser.Bitter/
  ```

The metainfo lists the old ID in `<provides>` and `<replaces>`, so software
centers treat the new ID as its successor.

## Maintainer guide

### Pipeline

The *Packages* workflow runs on pull requests that change packaging files and
from the *Release* workflow:

1. **source**: validates the desktop file, the metainfo and every packaging
   script (`shellcheck`), and makes the source archive with `git archive`.
2. **linux** (Debian 13 container): builds from the source archive
   (`packaging/linux/build-stage.sh`), then makes the `.deb`
   (`packaging/debian/build-deb.sh`, checked with `lintian`) and the portable
   archive (`packaging/linux/build-tarball.sh`).
3. **rpm** (Fedora 44 container): `packaging/rpm/build-rpm.sh` builds the spec
   from the source archive and runs `rpmlint`.
4. **arch** (Arch container): `packaging/arch/build-arch.sh` pins the release
   checksums into the three AUR recipes, builds `bitter-browser` and
   `bitter-browser-bin` with `makepkg`, runs `namcap`, writes each `.SRCINFO`
   and installs both packages. On a release, `packaging/arch/make-repo.sh`
   signs the package and builds the signed pacman repository.
5. **flatpak**: builds the bundle offline, installs it and checks the version.
6. **install**: installs each package on Debian 13, Ubuntu 26.04, Linux Mint
   23, Fedora 44, Fedora Rawhide, Arch and openSUSE Tumbleweed, checks the
   installed files, then removes it (`packaging/linux/test-install.sh`).
7. **smoke**: installs the `.deb` on Ubuntu 26.04 and drives the installed
   browser with `scripts/daily-smoke.py`.

The *Release* workflow then publishes the GitHub release with all assets,
checksums, SBOM and attestations; installs the published release with the
one-line installer and the pacman repository on each distribution
(`packaging/linux/verify-published.sh`); and only then pushes the three
recipes to the AUR with
[`KSXGitHub/github-actions-deploy-aur`](https://github.com/KSXGitHub/github-actions-deploy-aur),
which regenerates `.SRCINFO`. The first push creates each AUR package.

Building locally needs Docker; for example, the `.deb`:

```sh
docker run --rm -v "$PWD:/src:ro" -v "$PWD/out:/out" debian:trixie sh -c '
  cp -a /src /work && cd /work && sh packaging/linux/install-build-deps.sh &&
  . /root/.cargo/env && sh packaging/linux/build-stage.sh /out/stage &&
  sh packaging/debian/build-deb.sh 0.6.0 /out/stage /out'
```

### Secrets

The release needs these repository secrets (Settings > Secrets and
variables > Actions). The owner keeps the values in `.env` at the repository
root, which git ignores; `.env.example` lists the names.

| Secret | Value | Used by |
| --- | --- | --- |
| `AUR_USERNAME` | AUR account name | AUR job |
| `AUR_EMAIL` | AUR commit email | AUR job |
| `AUR_SSH_PRIVATE_KEY` | Private SSH key registered on the AUR account | AUR job |
| `PACMAN_GPG_PRIVATE_KEY` | Armored secret key of *Bitter Browser Packages* | arch job (`make-repo.sh`) |
| `PACMAN_GPG_FINGERPRINT` | `A22AFBDCDAA14B6958228038CAEA87029C2D9818` | arch job |
| `SITE_SSH_PRIVATE_KEY`, `SITE_SSH_KNOWN_HOSTS` | Hostinger SSH key and the server's host keys | *Website* workflow |

The *Website* workflow also reads the repository variables `SITE_SSH_HOST`,
`SITE_SSH_PORT`, `SITE_SSH_USER` and `SITE_REMOTE_DIR`.

The signing key expires on 2029-11-23. To rotate it, generate a new ed25519
signing key, replace `packaging/arch/bitter-browser-packages.asc`, update both
secrets and `.env`, and announce the new fingerprint in the release notes. If
the key leaks, import the revocation certificate kept in `.env`, publish the
revoked public key, and rotate.

### Version bump

`scripts/check-version.py` also checks `packaging/arch-bin/PKGBUILD` and
`packaging/rpm/bitter-browser.spec`. Bump them with the other version files
(see [CI, coverage and releases](ci-release.md#releases)).

### Flathub

Flathub needs a manifest in its own repository, reviewed by its maintainers.
Its [generative AI policy](https://docs.flathub.org/docs/for-app-authors/requirements)
says that AI tools must not write Flathub manifests or open or automate
submission pull requests, and that submitters must disclose AI-generated
material in the app. The owner submits the app: write the Flathub manifest by
hand (the in-repository manifest builds from a local directory; Flathub needs
the release archive and `cargo-sources.json`), disclose AI use, and open the
pull request against `flathub/flathub`. After acceptance, add the token from
the Flathub developer portal to
`https://browser.bebitterbebetter.com.br/.well-known/org.flathub.VerifiedApps.txt`
to verify the app.

### Not shipped yet

| Channel | Status |
| --- | --- |
| AppImage | Blocked. WebKitGTK 6.0 starts its helper processes from a path compiled into the library and always confines them with bubblewrap. A relocatable AppImage needs a private mount namespace (host `bwrap`) to keep that sandbox, which Ubuntu 24.04+ restricts with AppArmor. Shipping it with the sandbox off is not acceptable for a browser. |
| Flathub | Owner submission (above). |
| APT and DNF repositories, COPR, openSUSE Build Service, Launchpad PPA | Planned: they give automatic updates on Debian, Ubuntu, Fedora and openSUSE. |
| Snap Store | Not planned before Flathub; the GNOME snap extension would need its own WebKitGTK 6.0. |
| aarch64 | Planned: GitHub's arm64 runners can build every format. |
