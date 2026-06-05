// Package udev handles installation of udev rules for HID device access.
package udev

import (
	"fmt"
	"os"
	"os/exec"
	"runtime"
)

const rulesFilename = "99-corsair-nexus.rules"

// Rules is the udev rules content, embedded directly so the binary is
// self-contained — no dependency on the packaging directory at runtime.
const rules = `# udev rules for Corsair iCUE Nexus
# This allows non-root users in the plugdev group to access the device

# Corsair iCUE Nexus (VID: 0x1b1c, PID: 0x1b8e)
SUBSYSTEM=="usb", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="1b8e", MODE="0660", GROUP="plugdev", TAG+="uaccess"

# hidraw rule is required on Arch and any distro where hidraw ACLs are not
# inherited from the USB rule (e.g. distros using a custom udev ruleset).
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1b1c", ATTRS{idProduct}=="1b8e", MODE="0660", GROUP="plugdev", TAG+="uaccess"
`

// RulesInstalled reports whether the udev rules file is present on disk.
// Checks both the Arch path (/usr/lib) and the universal path (/etc).
func RulesInstalled() bool {
	for _, dir := range []string{"/usr/lib/udev/rules.d", "/etc/udev/rules.d"} {
		if _, err := os.Stat(dir + "/" + rulesFilename); err == nil {
			return true
		}
	}
	return false
}

// Setup installs the udev rules and reloads udev. Must be called as root.
// Returns a user-facing error message on failure.
func Setup() error {
	if runtime.GOOS != "linux" {
		return fmt.Errorf("udev setup is only supported on Linux")
	}

	if os.Getuid() != 0 {
		return fmt.Errorf(
			"root required — re-run with:\n\n  sudo nexus-open --setup-udev",
		)
	}

	rulesDir := detectRulesDir()
	if err := os.MkdirAll(rulesDir, 0755); err != nil {
		return fmt.Errorf("failed to create rules directory %s: %w", rulesDir, err)
	}

	dest := rulesDir + "/" + rulesFilename
	if err := os.WriteFile(dest, []byte(rules), 0644); err != nil {
		return fmt.Errorf("failed to write rules to %s: %w", dest, err)
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

	group := plugdevGroup()
	fmt.Printf("\n✓  Setup complete.\n\n")
	fmt.Printf("If this is your first install, add yourself to the %q group:\n\n", group)
	fmt.Printf("  sudo usermod -a -G %s $USER\n\n", group)
	fmt.Println("Then unplug and replug the iCUE Nexus — the device will be")
	fmt.Println("accessible without sudo from your next login.")

	return nil
}

// detectRulesDir returns the appropriate udev rules directory for this distro.
func detectRulesDir() string {
	// Arch Linux uses /usr/lib for package-managed rules.
	if _, err := os.Stat("/etc/arch-release"); err == nil {
		return "/usr/lib/udev/rules.d"
	}
	return "/etc/udev/rules.d"
}

// plugdevGroup returns the HID access group for this distro.
// Fedora/RHEL don't ship a plugdev group — they use 'input' instead.
func plugdevGroup() string {
	for _, f := range []string{"/etc/fedora-release", "/etc/redhat-release"} {
		if _, err := os.Stat(f); err == nil {
			return "input"
		}
	}
	return "plugdev"
}
