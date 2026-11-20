%{!?zendover:%global zendover 1.0.0}
%{!?tagversion:%global tagversion %{zendover}}

Name:           zendo
Version:        %{zendover}
Release:        1%{?dist}
Summary:        Every game in one library: Steam, Epic, GOG, Amazon and your own
License:        MIT AND OFL-1.1
URL:            https://github.com/leandrop-dev/zendo
Source0:        zendo-%{tagversion}.tar.gz
# cargo vendor --locked, made by packaging/fedora/make-srpm.sh
Source1:        zendo-%{tagversion}-vendor.tar.gz
# The frontend's built files, made from the same tree (COPR has no network)
Source2:        zendo-%{tagversion}-ui.tar.gz

ExclusiveArch:  x86_64

BuildRequires:  cargo-rpm-macros >= 28
BuildRequires:  rust >= 1.98
BuildRequires:  pkgconfig(gtk+-3.0)
BuildRequires:  pkgconfig(webkit2gtk-4.1)
BuildRequires:  pkgconfig(libsoup-3.0)
BuildRequires:  pkgconfig(javascriptcoregtk-4.1)
BuildRequires:  pkgconfig(libudev)
BuildRequires:  desktop-file-utils
# %%check: the tests make and unpack archives and run checksums
BuildRequires:  tar
BuildRequires:  xz
BuildRequires:  bsdtar
BuildRequires:  7zip
BuildRequires:  coreutils

# The window is a WebKitGTK 4.1 web view on GTK 3. WebKitGTK loads GL, Wayland,
# X11 and fontconfig for itself, so rpm cannot see them from the binary.
Requires:       webkit2gtk4.1
Requires:       gtk3
Requires:       libsoup3
Requires:       fontconfig
Requires:       hicolor-icon-theme
# Settings -> Engines downloads and unpacks the engines and Protons
Requires:       python3
Requires:       tar
Requires:       xz
Requires:       coreutils
# Unpacks mods and GOG Galaxy's dummy-service.zip
Requires:       (7zip or bsdtar)
Recommends:     xdg-utils
Suggests:       steam
Suggests:       gamescope
Suggests:       mangohud
Suggests:       gamemode

%description
Zendo is a game launcher: one library for Steam, Epic, GOG and Amazon games
and games installed by hand, launched with the right tools (Proton through
umu-launcher, MangoHud, GameMode, gamescope), with covers, achievements and
mods. The store engines (legendary, gogdl, nile, comet), umu-launcher and
the Protons are downloaded from Settings, not packaged here.

%prep
%autosetup -n zendo-%{tagversion} -a1
tar xzf %{SOURCE2} -C crates/zendo/ui
%cargo_prep -v vendor

%build
%cargo_build
%{cargo_vendor_manifest}
# the workspace root is a virtual manifest; the licences are the app's
(cd crates/zendo && %{cargo_license_summary})
(cd crates/zendo && %{cargo_license}) > LICENSE.dependencies

%install
install -Dpm0755 target/rpm/zendo %{buildroot}%{_bindir}/zendo
install -Dpm0644 data/zendo.desktop %{buildroot}%{_datadir}/applications/zendo.desktop
for size in 16 22 24 32 48 64 128 256 512; do
    install -Dpm0644 data/icons/zendo-$size.png \
        %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/zendo.png
done

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/zendo.desktop
%cargo_test

%files
%license LICENSE
%license LICENSE.dependencies
%license cargo-vendor.txt
%license crates/zendo/ui/static/fonts/OFL.txt
%doc README.md
%{_bindir}/zendo
%{_datadir}/applications/zendo.desktop
%{_datadir}/icons/hicolor/*/apps/zendo.png

%changelog
* Tue Nov 17 2026 Leandro Paixão <leo@codemonk.eu> - 1.0.0-1
- First stable release
