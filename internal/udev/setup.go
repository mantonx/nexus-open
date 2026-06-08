// Package udev handles installation of udev rules for HID device access.
package udev

import (
	"fmt"
	"os"
	"os/exec"
	"runtime"
)

const rulesFilename = "99-corsair-nexus.rules"

// rules is embedded directly so the binary is self-contained — no dependency
// on the packaging directory at runtime.
const rules = `# udev rules for Corsair iCUE Nexus (VID: 0x1b1c, PID: 0x1b8e)
#
# TAG+="uaccess" grants access to the active desktop session automatically
# via logind — this is the primary access mechanism on all systemd desktops.
# No group membership is required for normal desktop use.
#
# MODE="0600" is a root-only fallback: it prevents world-read while still
# allowing logind/uaccess to set per-session ACLs.  On desktops, uaccess
# is sufficient; users should NOT need to join plugdev.
#
# GROUP="plugdev" + MODE="0660" is only needed on headless or multi-user
# setups where no logind session is active (e.g. a server running the daemon
# under a dedicated service account).  Enable it manually in that case.
#
# RUN+="usbreset" issues a USB reset on each attach so any stale usbfs handle
# from a previous unclean exit is cleared and hid-generic rebinds cleanly.
# Requires the usbreset binary (shipped in the nexus-open package).

SUBSYSTEM=="usb", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="1b8e", \
    MODE="0600", TAG+="uaccess", \
    RUN+="/usr/bin/usbreset /dev/bus/usb/$env{BUSNUM}/$env{DEVNUM}"

# hidraw nodes do not always inherit ACLs from the parent USB device
# (distro-dependent). This rule ensures direct hidraw access works everywhere.
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="1b8e", \
    MODE="0600", TAG+="uaccess"
`

// RulesInstalled reports whether the udev rules file is present on disk.
// Checks both common locations.
func RulesInstalled() bool {
	for _, dir := range []string{"/usr/lib/udev/rules.d", "/etc/udev/rules.d"} {
		if _, err := os.Stat(dir + "/" + rulesFilename); err == nil {
			return true
		}
	}
	return false
}

// Setup installs the udev rules and reloads udev. Must be called as root.
func Setup() error {
	if runtime.GOOS != "linux" {
		return fmt.Errorf("udev setup is only supported on Linux")
	}

	if os.Getuid() != 0 {
		return fmt.Errorf(
			"root required — re-run with:\n\n  sudo nexus-open --setup-udev",
		)
	}

	// Ensure plugdev group exists — needed as fallback for headless installs
	// where TAG+="uaccess"/logind is not in play.
	if err := ensureGroup("plugdev"); err != nil {
		return err
	}

	rulesDir := detectRulesDir()
	if err := os.MkdirAll(rulesDir, 0755); err != nil {
		return fmt.Errorf("failed to create rules directory %s: %w", rulesDir, err)
	}

	dest := rulesDir + "/" + rulesFilename
	if err := os.WriteFile(dest, []byte(rules), 0644); err != nil {
		return fmt.Errorf("failed to write %s: %w", dest, err)
	}
	fmt.Printf("  ✓  Written: %s\n", dest)

	// Reload udev and trigger rule matching on existing devices.
	for _, args := range [][]string{
		{"udevadm", "control", "--reload-rules"},
		{"udevadm", "trigger", "--subsystem-match=usb"},
		{"udevadm", "trigger", "--subsystem-match=hidraw"},
	} {
		if out, err := exec.Command(args[0], args[1:]...).CombinedOutput(); err != nil {
			return fmt.Errorf("failed to run %v: %w\n%s", args, err, out)
		}
	}
	fmt.Println("  ✓  udev rules reloaded")

	fmt.Printf("\n✓  Setup complete.\n\n")
	fmt.Println("Unplug and replug your iCUE Nexus — it will be accessible")
	fmt.Println("immediately in your current desktop session (no logout needed).")
	fmt.Println()
	fmt.Println("Headless / multi-user systems only:")
	fmt.Println("  If running without a logind session (e.g. a dedicated service account),")
	fmt.Println("  change MODE to 0660, add GROUP=\"plugdev\", and add that user to plugdev:")
	fmt.Println()
	fmt.Println("  sudo usermod -a -G plugdev <serviceuser>")
	fmt.Println()
	fmt.Println("Desktop users do not need to join plugdev.")

	return nil
}

// detectRulesDir returns the best udev rules directory for this system.
// Prefers /usr/lib/udev/rules.d when it exists (Arch, openSUSE, and other
// distros that treat /usr/lib as the package-managed location). Falls back
// to /etc/udev/rules.d which works on all distros.
func detectRulesDir() string {
	if _, err := os.Stat("/usr/lib/udev/rules.d"); err == nil {
		return "/usr/lib/udev/rules.d"
	}
	return "/etc/udev/rules.d"
}

// ensureGroup creates the named group if it doesn't exist.
func ensureGroup(name string) error {
	if out, err := exec.Command("getent", "group", name).Output(); err == nil && len(out) > 0 {
		return nil // already exists
	}
	if out, err := exec.Command("groupadd", "-r", name).CombinedOutput(); err != nil {
		return fmt.Errorf("failed to create group %q: %w\n%s", name, err, out)
	}
	fmt.Printf("  ✓  Created group: %s\n", name)
	return nil
}
