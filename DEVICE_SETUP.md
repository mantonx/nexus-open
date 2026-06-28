# Corsair iCUE Nexus — Device Setup

This guide covers USB permission setup for all major Linux distributions so that
Nexus Open can access the device without `sudo`.

---

## Quick Setup (All Distros)

Run the included setup script — it detects your distro and writes the rules to
the correct location:

```bash
sudo ./scripts/setup-udev.sh
```

Then either unplug and replug the device, or run:

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

---

## Ubuntu / Debian / Linux Mint

1. Install the udev rules:
   ```bash
   sudo cp packaging/udev/99-corsair-nexus.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```

2. Add your user to `plugdev`:
   ```bash
   sudo usermod -a -G plugdev $USER
   ```

3. Log out and back in. The group change takes effect at the next login.

> **Note:** `TAG+="uaccess"` in the rules file means desktop session users
> on systemd-logind systems (most distros) get access automatically without
> manual group assignment. The `plugdev` step is a belt-and-suspenders fallback.

---

## Arch Linux

1. Install the udev rules to the package-managed path:
   ```bash
   sudo cp packaging/udev/99-corsair-nexus.rules /usr/lib/udev/rules.d/
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```
   > Use `/usr/lib/udev/rules.d/` on Arch, not `/etc/udev/rules.d/` — rules in
   > `/usr/lib/` are managed by the package manager; `/etc/` overrides are for
   > local customisation only.

2. Add your user to `plugdev` (create the group if it doesn't exist):
   ```bash
   sudo groupadd -r plugdev 2>/dev/null || true
   sudo usermod -a -G plugdev $USER
   ```

3. Log out and back in.

4. Verify the device is accessible:
   ```bash
   ls -l /dev/hidraw* | grep -i "1b1c\|hidraw"
   # Should show group-readable entries
   ```

If you installed via the AUR package, the rules are placed automatically during
`post_install`. Run `sudo udevadm control --reload-rules && sudo udevadm trigger`
after the first install.

---

## Fedora / RHEL / CentOS / openSUSE

On Fedora and Red Hat-based systems, use the `input` group instead of `plugdev`
(which does not exist by default):

1. Install the udev rules:
   ```bash
   sudo cp packaging/udev/99-corsair-nexus.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```

2. Add your user to `input`:
   ```bash
   sudo usermod -a -G input $USER
   ```

3. Log out and back in.

**SELinux note:** If SELinux is enforcing (default on Fedora/RHEL), the
`TAG+="uaccess"` tag in the rules grants access to the active desktop session
user via logind automatically. Manual group assignment is usually not required
in a normal desktop session.

If you still get permission errors with SELinux enforcing, check AVC denials:

```bash
sudo ausearch -m avc -ts recent | grep hidraw
```

---

## Verifying Access

After setup, confirm the device is accessible without `sudo`:

```bash
# Find the Nexus hidraw node
for d in /dev/hidraw*; do
    udevadm info "$d" 2>/dev/null | grep -q "1b1c" && echo "$d"
done
```

Then run the app:

```bash
./bin/nexus-open
```

You should see `level=INFO msg="HID device connected"` in the log. If you still
see `failed to open any HID interface`, check the actionable message in
`GET /api/device/info` — it will tell you whether the error is permissions,
device not found, or device busy.

---

## Troubleshooting

### Device not found

Run `lsusb | grep 1b1c` to confirm the device is visible to the OS. If it shows
up but the daemon can't open it, run `make doctor` to check USB permissions.

### Port 1985 in use

```bash
ss -tlnp | grep 1985
```

Or start the daemon on a different port: `nexus-open --port 1986`.

### Plugin shows blank

Check `GET /api/zones/{id}/status` for the error message.

### Permission denied after running setup-udev.sh

Unplug and replug the USB cable — udev rules apply to newly-connected devices.
Running `sudo udevadm trigger` re-applies rules to already-connected devices but
occasionally misses hidraw nodes. A replug is the reliable fix.

### Which `/dev/hidraw*` node is the Nexus?

```bash
for d in /dev/hidraw*; do
    echo "=== $d ==="
    udevadm info "$d" | grep -E "ID_VENDOR_ID|ID_MODEL_ID"
done
```

Look for `ID_VENDOR_ID=1b1c` and `ID_MODEL_ID=1b8e`.

### Multiple HID interfaces listed in the log

This is normal. The Nexus exposes several HID interfaces; the driver tries each
in turn and uses the first one that opens successfully.

---

## Device Information

| Field     | Value                            |
|-----------|----------------------------------|
| Vendor    | Corsair (1b1c)                   |
| Product   | iCUE Nexus (1b8e)                |
| Display   | 640×48 px ultra-wide touchscreen |
| Interface | USB HID                          |
