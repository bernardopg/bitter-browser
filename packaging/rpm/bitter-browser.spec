# Fedora package recipe. Builds the release source archive with Meson.
# The build downloads the Rust crates pinned in Cargo.lock (cargo --locked),
# so mock and COPR builds need network access enabled.
%global app_id br.com.bebitterbebetter.browser.Bitter
# The release profile builds without debug information.
%global debug_package %{nil}

Name:           bitter-browser
Version:        0.6.2
Release:        1%{?dist}
Summary:        Focused, native web browser for the Linux desktop
License:        MPL-2.0
URL:            https://browser.bebitterbebetter.com.br/
Source0:        https://github.com/bernardopg/bitter-browser/releases/download/v%{version}/%{name}-%{version}.tar.gz
ExclusiveArch:  x86_64 aarch64

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  meson
BuildRequires:  pkgconfig(gtk4) >= 4.14
BuildRequires:  pkgconfig(libadwaita-1) >= 1.6
BuildRequires:  pkgconfig(webkitgtk-6.0) >= 2.44
BuildRequires:  desktop-file-utils
BuildRequires:  appstream
BuildRequires:  binutils
Requires:       hicolor-icon-theme
Recommends:     gstreamer1-plugins-good
Recommends:     gstreamer1-plugins-bad-free
Recommends:     xdg-desktop-portal
Recommends:     xdg-utils
Suggests:       hunspell-en

%description
Bitter Browser is a web browser built with Rust, GTK 4, libadwaita and
WebKitGTK. It keeps tabs in a vertical sidebar organized in two workspaces,
restores sessions, and blocks ads and trackers.

%prep
%autosetup

%build
%meson
%meson_build

%install
%meson_install
strip --strip-unneeded %{buildroot}%{_bindir}/bitter-ui %{buildroot}%{_bindir}/bitter-cli

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/%{app_id}.desktop
appstreamcli validate --no-net %{buildroot}%{_metainfodir}/%{app_id}.metainfo.xml
%{buildroot}%{_bindir}/bitter-cli --version

%files
%license %{_datadir}/licenses/%{name}/LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/bitter-ui
%{_bindir}/bitter-cli
%{_datadir}/applications/%{app_id}.desktop
%{_metainfodir}/%{app_id}.metainfo.xml
%{_datadir}/glib-2.0/schemas/%{app_id}.gschema.xml
%{_datadir}/icons/hicolor/*/apps/%{app_id}*
%dir %{_datadir}/xfce4
%dir %{_datadir}/xfce4/helpers
%{_datadir}/xfce4/helpers/bitter-browser.desktop

%changelog
* Wed Nov 25 2026 Bitter Browser Contributors <bernardopg@users.noreply.github.com> - 0.6.2-1
- Republishes 0.6.1, whose release workflow could not start

* Wed Nov 25 2026 Bitter Browser Contributors <bernardopg@users.noreply.github.com> - 0.6.1-1
- Flatpak default browser fix, new screenshots and release pipeline fixes

* Tue Nov 24 2026 Bitter Browser Contributors <bernardopg@users.noreply.github.com> - 0.6.0-1
- Native packages, default browser setting, new application ID and visual identity

* Tue Nov 24 2026 Bitter Browser Contributors <bernardopg@users.noreply.github.com> - 0.5.0-1
- See https://github.com/bernardopg/bitter-browser/blob/main/CHANGELOG.md
