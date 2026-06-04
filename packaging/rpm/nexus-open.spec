Name:           nexus-open
Version:        1.0.0
Release:        1%{?dist}
Summary:        Open-source Linux controller for the Corsair iCUE Nexus display

License:        MIT
URL:            https://github.com/mantonx/nexus-next
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  golang >= 1.23
BuildRequires:  libusb1-devel
BuildRequires:  systemd-devel
BuildRequires:  libayatana-appindicator3-devel
Requires:       libusb1
Requires:       libayatana-appindicator3

%description
Nexus Open provides a native Linux desktop application to control and
configure the Corsair iCUE Nexus (640x48 pixel display). Displays system
metrics, weather information, and custom backgrounds on the device.

Features:
  - Real-time CPU/GPU temperature and network statistics
  - Weather display with configurable units
  - Custom background images
  - Touch input support
  - Flutter-based configuration UI
  - REST API for programmatic control

%prep
%setup -q

%build
export CGO_ENABLED=1
go build \
    -trimpath \
    -ldflags "-X main.version=%{version} -X main.commit=$(git rev-parse --short HEAD 2>/dev/null || echo unknown)" \
    -o %{name} \
    ./cmd/nexus-open

%install
install -Dm755 %{name} %{buildroot}%{_bindir}/%{name}
install -Dm644 packaging/desktop/nexus-open.desktop \
    %{buildroot}%{_datadir}/applications/nexus-open.desktop
install -Dm644 packaging/udev/99-corsair-nexus.rules \
    %{buildroot}%{_udevrulesdir}/99-corsair-nexus.rules
install -Dm644 packaging/systemd/nexus-open.service \
    %{buildroot}%{_userunitdir}/nexus-open.service

# Icons (install whichever sizes exist)
for size in 16 48 64 128 256; do
    icon="packaging/icons/${size}.png"
    if [ -f "${icon}" ]; then
        install -Dm644 "${icon}" \
            %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/nexus-open.png
    fi
done

%post
udevadm control --reload-rules 2>/dev/null || true
udevadm trigger --subsystem-match=usb 2>/dev/null || true
udevadm trigger --subsystem-match=hidraw 2>/dev/null || true

%files
%license LICENSE
%doc README.md
%{_bindir}/%{name}
%{_datadir}/applications/nexus-open.desktop
%{_udevrulesdir}/99-corsair-nexus.rules
%{_userunitdir}/nexus-open.service
%{_datadir}/icons/hicolor/*/apps/nexus-open.png

%changelog
* Wed Jun 04 2026 Matt <matthew.panton@gmail.com> - 1.0.0-1
- Initial RPM release
