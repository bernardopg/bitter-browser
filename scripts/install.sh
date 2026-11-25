#!/bin/sh
# Bitter Browser installer.
#
#   curl -fsSL https://browser.bebitterbebetter.com.br/install.sh | sh
#   curl -fsSL https://browser.bebitterbebetter.com.br/install.sh | sh -s -- --method flatpak
#
# Options:
#   --method auto|deb|rpm|pacman|tarball|flatpak   (default: auto)
#   --version X.Y.Z    install this release instead of the latest one
#   --uninstall        remove what this method installed
#   --dry-run          print the commands without running them
#
# auto picks the native package for Debian/Ubuntu (.deb), Fedora (.rpm) and
# Arch Linux (pacman), and the portable archive in ~/.local elsewhere.
# Every download is checked against the release's SHA-256 checksum file
# before anything is installed. Read this script before piping it to sh.
set -eu

repo=bernardopg/bitter-browser
app_id=br.com.bebitterbebetter.browser.Bitter
method=auto
version=
uninstall=0
dry_run=0

say() { printf '%s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }
run() {
    say "+ $*"
    [ "$dry_run" = 1 ] || "$@"
}

while [ $# -gt 0 ]; do
    case $1 in
        --method) method=${2:?--method needs a value}; shift 2 ;;
        --method=*) method=${1#--method=}; shift ;;
        --version) version=${2:?--version needs a value}; shift 2 ;;
        --version=*) version=${1#--version=}; shift ;;
        --uninstall) uninstall=1; shift ;;
        --dry-run) dry_run=1; shift ;;
        -h|--help) sed -n '2,17p' "$0" 2>/dev/null || true; exit 0 ;;
        *) die "unknown option $1" ;;
    esac
done
version=${version#v}

[ "$(uname -s)" = Linux ] || die "Bitter Browser runs on Linux only."
machine=$(uname -m)
[ "$machine" = x86_64 ] || [ "$method" = flatpak ] ||
    die "release packages are built for x86_64; on $machine build from source."

need() { command -v "$1" >/dev/null 2>&1; }
if [ "$(id -u)" = 0 ]; then sudo=; elif need sudo; then sudo=sudo; elif need doas; then sudo=doas; else sudo=; fi
as_root() {
    if [ "$(id -u)" = 0 ] || [ -z "$sudo" ]; then run "$@"; else run "$sudo" "$@"; fi
}

distro_family() {
    [ -r /etc/os-release ] || { echo other; return; }
    # shellcheck disable=SC1091
    ids=$(. /etc/os-release; printf '%s %s' "${ID:-}" "${ID_LIKE:-}")
    case " $ids " in
        *" arch "*|*" archlinux "*) echo arch ;;
        *" fedora "*|*" rhel "*) echo fedora ;;
        *" debian "*|*" ubuntu "*) echo debian ;;
        *) echo other ;;
    esac
}

if [ "$method" = auto ]; then
    case $(distro_family) in
        arch) method=pacman ;;
        fedora) method=rpm ;;
        debian) method=deb ;;
        *) method=tarball ;;
    esac
fi
case $method in deb|rpm|pacman|tarball|flatpak) ;; *) die "unknown method $method" ;; esac

if [ "$uninstall" = 1 ]; then
    case $method in
        deb) as_root apt-get remove -y bitter-browser ;;
        rpm) as_root dnf remove -y bitter-browser ;;
        pacman) as_root pacman -R --noconfirm bitter-browser ;;
        flatpak) run flatpak uninstall -y "$app_id" ;;
        tarball)
            if [ -x "$HOME/.local/share/bitter-browser/install.sh" ]; then
                run "$HOME/.local/share/bitter-browser/install.sh" --uninstall
                run rm -rf "$HOME/.local/share/bitter-browser"
            else
                die "no portable install found in ~/.local"
            fi ;;
    esac
    exit 0
fi

need curl || die "curl is required."
need sha256sum || die "sha256sum (coreutils) is required."

if [ -z "$version" ]; then
    # /releases/latest redirects to /releases/tag/vX.Y.Z.
    latest=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest")
    version=${latest##*/v}
    case $version in [0-9]*.[0-9]*.[0-9]*) ;; *) die "could not find the latest release" ;; esac
fi
base=https://github.com/$repo/releases/download/v$version
say "Installing Bitter Browser $version ($method)"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM
fetch() {
    say "Downloading $1"
    curl -fL --proto '=https' --tlsv1.2 --retry 3 -o "$tmp/$1" "$base/$1"
}
verify() {
    grep -E "^[0-9a-f]{64}  $1\$" "$tmp/sums" > "$tmp/one" || die "$1 is not in the checksum file"
    (cd "$tmp" && sha256sum -c one) || die "checksum mismatch for $1; nothing was installed"
}
curl -fsSL --proto '=https' --tlsv1.2 -o "$tmp/sums" "$base/bitter-browser-$version-sha256.txt" ||
    die "release v$version not found"

case $method in
    deb)
        file=bitter-browser_${version}-1_amd64.deb
        fetch "$file"; verify "$file"
        as_root apt-get install -y "$tmp/$file"
        ;;
    rpm)
        file=$(sed -n "s/^[0-9a-f]\{64\}  \(bitter-browser-$version-[0-9]*\.[a-z0-9_]*\.x86_64\.rpm\)$/\1/p" "$tmp/sums" | head -n 1)
        [ -n "$file" ] || die "no x86_64 RPM in release v$version"
        fetch "$file"; verify "$file"
        if need dnf; then as_root dnf install -y "$tmp/$file"
        elif need zypper; then as_root zypper --non-interactive install --allow-unsigned-rpm "$tmp/$file"
        else die "dnf or zypper is required"; fi
        ;;
    pacman)
        file=bitter-browser-$version-1-x86_64.pkg.tar.zst
        fetch "$file"; verify "$file"
        as_root pacman -U --noconfirm "$tmp/$file"
        ;;
    flatpak)
        need flatpak || die "flatpak is not installed; see https://flatpak.org/setup/"
        file=bitter-browser-$version-x86_64.flatpak
        fetch "$file"; verify "$file"
        run flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
        run flatpak install --user -y --noninteractive "$tmp/$file"
        ;;
    tarball)
        file=bitter-browser-$version-linux-x86_64.tar.xz
        fetch "$file"; verify "$file"
        run tar -xJf "$tmp/$file" -C "$tmp"
        run "$tmp/bitter-browser-$version-linux-x86_64/install.sh"
        # Keep the installer so --uninstall works later.
        run install -Dm755 "$tmp/bitter-browser-$version-linux-x86_64/install.sh" \
            "$HOME/.local/share/bitter-browser/install.sh"
        ;;
esac

say ""
say "Bitter Browser $version is installed."
say "Make it your default browser in Preferences > General > Default browser,"
say "or run: xdg-settings set default-web-browser $app_id.desktop"
