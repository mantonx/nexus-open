# Nexus Open, the Rust app: nexusd (the daemon and, as `nexusd ui`, the
# settings window) and nexusctl.
#
# From source:     rpmbuild -ba nexus-open.spec   (Source0 in SOURCES; needs
#                  cargo 1.100 or newer and network access for crates)
# From binaries:   rpmbuild -bb nexus-open.spec --define "prebuilt /path/to/bin"
#                  where /path/to/bin holds nexusd and nexusctl; the release
#                  workflow does this with binaries built on Ubuntu 24.04.
#
# The package keeps the old Go app's name, so it replaces it in place.

# The binaries come from cargo, not from this build's compiler flags.
%global debug_package %{nil}
# Fedora's systemd-rpm-macros define these; rpmbuild elsewhere (the release
# workflow runs on Ubuntu) does not.
%{!?_userunitdir: %global _userunitdir %{_prefix}/lib/systemd/user}
%{!?_userpresetdir: %global _userpresetdir %{_prefix}/lib/systemd/user-preset}
%{!?_udevrulesdir: %global _udevrulesdir %{_prefix}/lib/udev/rules.d}
%{!?_metainfodir: %global _metainfodir %{_datadir}/metainfo}

Name:           nexus-open
Version:        0.1.0
Release:        1%{?dist}
Summary:        Drives the Corsair iCUE Nexus touch strip, with a settings window
# MIT for the app; the fonts compiled into it are OFL-1.1, MIT (Phosphor)
# and Bitstream-Vera (DejaVu Sans).
License:        MIT AND OFL-1.1 AND Bitstream-Vera
URL:            https://github.com/mantonx/nexus-open
Source0:        %{url}/archive/refs/tags/v%{version}/%{name}-%{version}.tar.gz

%if %{undefined prebuilt}
BuildRequires:  cargo >= 1.100
BuildRequires:  gcc
BuildRequires:  pkgconfig(fontconfig)
BuildRequires:  systemd-rpm-macros
%endif

Requires:       hicolor-icon-theme
# Loaded at run time (not linked), so rpm cannot find it by itself.
Requires:       libxkbcommon.so.0()(64bit)
Requires:       systemd
Requires:       systemd-udev
# Loaded at run time by the settings window (Wayland or X11, OpenGL with a
# software fallback).
Recommends:     libwayland-client
Recommends:     libwayland-cursor
Recommends:     libwayland-egl
Recommends:     libglvnd-egl
Recommends:     libglvnd-glx
Recommends:     libX11
Recommends:     libX11-xcb
Recommends:     libXcursor
Recommends:     libXi
Recommends:     libXrandr
Recommends:     libxkbcommon-x11

%description
Nexus Open shows the time, readings from the computer, the weather and what
is playing on the Corsair iCUE Nexus touch strip, and puts media, volume and
launchers under a finger. A user service draws the strip from login; the
settings window (nexusd ui) edits it with a live preview.

Not made or endorsed by Corsair.

%prep
%autosetup -n %{name}-%{version}

%build
%if %{undefined prebuilt}
cargo build --release --locked -p nexusd -p nexusctl
%endif

%install
packaging/nexusd/stage.sh %{buildroot} %{?prebuilt}%{!?prebuilt:target/release}

%post
# The work is in package-hook, shared with the Arch and deb packages.
if [ "$1" -eq 1 ]; then
    %{_prefix}/lib/nexus-open/package-hook install
else
    %{_prefix}/lib/nexus-open/package-hook upgrade
fi

%preun
if [ "$1" -eq 0 ]; then
    %{_prefix}/lib/nexus-open/package-hook remove
fi

%postun
if [ "$1" -eq 0 ]; then
    udevadm control --reload-rules 2>/dev/null || true
fi

%files
%license %{_datadir}/licenses/%{name}/LICENSE
%license %{_datadir}/licenses/%{name}/fonts
%{_bindir}/nexusd
%{_bindir}/nexusctl
%{_userunitdir}/nexusd.service
%{_userpresetdir}/80-nexus-open.preset
%{_udevrulesdir}/70-nexus-open.rules
%{_datadir}/applications/io.nexus.open.desktop
%{_metainfodir}/io.nexus.open.metainfo.xml
%{_datadir}/icons/hicolor/256x256/apps/io.nexus.open.png
%dir %{_prefix}/lib/nexus-open
%{_prefix}/lib/nexus-open/package-hook

%changelog
* Sun Nov 22 2026 Matt <matthew.panton@gmail.com> - 0.1.0-1
- First release of the Rust app.
