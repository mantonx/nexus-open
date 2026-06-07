package device

import (
	"os"
	"path/filepath"
	"testing"
	"unsafe"

	"golang.org/x/sys/unix"
)

// TestUsbfsStructSizes asserts the exact byte sizes the kernel expects for
// each ioctl struct. A wrong field width or missing padding silently corrupts
// every USB transfer, so these are the highest-value tests in this file —
// they catch ABI breakage at go test time with no hardware required.
func TestUsbfsStructSizes(t *testing.T) {
	tests := []struct {
		name string
		got  uintptr
		want uintptr
	}{
		// struct usbdevfs_bulktransfer: ep(4) + length(4) + timeout(4) + pad(4) + data(8) = 24
		{"bulkTransfer", unsafe.Sizeof(bulkTransfer{}), 24},
		// struct usbdevfs_ctrltransfer: bRequestType(1)+bRequest(1)+wValue(2)+wIndex(2)+wLength(2)+timeout(4)+pad(4)+data(8) = 24
		{"ctrlTransfer", unsafe.Sizeof(ctrlTransfer{}), 24},
		// struct usbdevfs_ioctl: ifno(4) + ioctl_code(4) + data(8) = 16
		{"usbfsIoctl", unsafe.Sizeof(usbfsIoctl{}), 16},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.got != tt.want {
				t.Errorf("sizeof(%s) = %d, want %d — ioctl ABI broken", tt.name, tt.got, tt.want)
			}
		})
	}
}

// TestFindDevice_MatchesByVIDPID verifies that findDevice returns the correct
// bus/device numbers and string descriptors when a matching device is present
// in a fake sysfs tree.
func TestFindDevice_MatchesByVIDPID(t *testing.T) {
	root := makeFakeSysfs(t, []fakeDevice{
		{name: "1-1", vid: "1b1c", pid: "1b8e", bus: "1", dev: "3", mfr: "Corsair", prod: "iCUE NEXUS"},
	})
	origSysfsRoot := sysfsUSBDevices
	sysfsUSBDevices = root
	defer func() { sysfsUSBDevices = origSysfsRoot }()

	bus, dev, mfr, prod, err := findDevice(0x1b1c, 0x1b8e)
	if err != nil {
		t.Fatalf("findDevice returned unexpected error: %v", err)
	}
	if bus != 1 || dev != 3 {
		t.Errorf("got bus=%d dev=%d, want bus=1 dev=3", bus, dev)
	}
	if mfr != "Corsair" {
		t.Errorf("got manufacturer=%q, want %q", mfr, "Corsair")
	}
	if prod != "iCUE NEXUS" {
		t.Errorf("got product=%q, want %q", prod, "iCUE NEXUS")
	}
}

// TestFindDevice_SkipsNonMatchingEntries verifies that devices with a
// different VID or PID are not returned.
func TestFindDevice_SkipsNonMatchingEntries(t *testing.T) {
	root := makeFakeSysfs(t, []fakeDevice{
		{name: "1-1", vid: "046d", pid: "c52b", bus: "1", dev: "2", mfr: "Logitech", prod: "Unifying"},
		{name: "1-2", vid: "1b1c", pid: "1b8e", bus: "1", dev: "4", mfr: "Corsair", prod: "iCUE NEXUS"},
	})
	origSysfsRoot := sysfsUSBDevices
	sysfsUSBDevices = root
	defer func() { sysfsUSBDevices = origSysfsRoot }()

	bus, dev, _, _, err := findDevice(0x1b1c, 0x1b8e)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if bus != 1 || dev != 4 {
		t.Errorf("got bus=%d dev=%d, want bus=1 dev=4", bus, dev)
	}
}

// TestFindDevice_NotFound verifies the error returned when no device matches.
func TestFindDevice_NotFound(t *testing.T) {
	root := makeFakeSysfs(t, []fakeDevice{
		{name: "1-1", vid: "046d", pid: "c52b", bus: "1", dev: "2"},
	})
	origSysfsRoot := sysfsUSBDevices
	sysfsUSBDevices = root
	defer func() { sysfsUSBDevices = origSysfsRoot }()

	_, _, _, _, err := findDevice(0x1b1c, 0x1b8e)
	if err == nil {
		t.Fatal("expected error for missing device, got nil")
	}
}

// TestFindDevice_EmptySysfs verifies the error when the sysfs tree is empty.
func TestFindDevice_EmptySysfs(t *testing.T) {
	root := t.TempDir()
	origSysfsRoot := sysfsUSBDevices
	sysfsUSBDevices = root
	defer func() { sysfsUSBDevices = origSysfsRoot }()

	_, _, _, _, err := findDevice(0x1b1c, 0x1b8e)
	if err == nil {
		t.Fatal("expected error for empty sysfs, got nil")
	}
}

// TestReadSysfs_MissingFile verifies that readSysfs returns "" for absent attrs.
func TestReadSysfs_MissingFile(t *testing.T) {
	dir := t.TempDir()
	got := readSysfs(dir, "nonexistent")
	if got != "" {
		t.Errorf("got %q, want empty string for missing attr", got)
	}
}

// TestReadSysfs_TrimsWhitespace verifies trailing newlines are stripped.
func TestReadSysfs_TrimsWhitespace(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "attr"), []byte("1b1c\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	got := readSysfs(dir, "attr")
	if got != "1b1c" {
		t.Errorf("got %q, want %q", got, "1b1c")
	}
}

// TestReadSysfsInt_Valid verifies decimal integer parsing.
func TestReadSysfsInt_Valid(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "busnum"), []byte("3\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	got := readSysfsInt(dir, "busnum")
	if got != 3 {
		t.Errorf("got %d, want 3", got)
	}
}

// TestReadSysfsInt_Malformed verifies that non-integer content returns 0.
func TestReadSysfsInt_Malformed(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "busnum"), []byte("not-a-number\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	got := readSysfsInt(dir, "busnum")
	if got != 0 {
		t.Errorf("got %d, want 0 for malformed int", got)
	}
}

// TestIsTimeout verifies the two errno values that represent a USB timeout.
func TestIsTimeout(t *testing.T) {
	if !isTimeout(unix.ETIMEDOUT) {
		t.Error("ETIMEDOUT should be a timeout")
	}
	if !isTimeout(unix.ENODATA) {
		t.Error("ENODATA should be a timeout")
	}
	if isTimeout(unix.EIO) {
		t.Error("EIO should not be a timeout")
	}
	if isTimeout(nil) {
		t.Error("nil should not be a timeout")
	}
}

// fakeDevice describes one entry to write into the fake sysfs tree.
type fakeDevice struct {
	name string
	vid  string
	pid  string
	bus  string
	dev  string
	mfr  string
	prod string
}

// makeFakeSysfs creates a temporary directory tree that mimics
// /sys/bus/usb/devices and returns its path.
func makeFakeSysfs(t *testing.T, devices []fakeDevice) string {
	t.Helper()
	root := t.TempDir()
	for _, d := range devices {
		dir := filepath.Join(root, d.name)
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
		writeAttr := func(attr, val string) {
			if val == "" {
				return
			}
			if err := os.WriteFile(filepath.Join(dir, attr), []byte(val+"\n"), 0o644); err != nil {
				t.Fatal(err)
			}
		}
		writeAttr("idVendor", d.vid)
		writeAttr("idProduct", d.pid)
		writeAttr("busnum", d.bus)
		writeAttr("devnum", d.dev)
		writeAttr("manufacturer", d.mfr)
		writeAttr("product", d.prod)
	}
	return root
}
