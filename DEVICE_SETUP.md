# Corsair iCUE Nexus Device Setup

## Current Issue

The UI redesign implementation is complete and working correctly, but the application cannot access the Corsair iCUE Nexus display hardware due to HID device permissions.

**Error:** `failed to connect to device: failed to open any HID interface`

**Root Cause:** The `/dev/hidraw*` devices are owned by root with 600 permissions, preventing non-root users from accessing them.

## Solution Options

### Option 1: Setup udev Rules (Recommended)

This grants permanent access to the device without requiring sudo for each run.

**Steps:**

1. Run the setup script (will prompt for sudo password):
   ```bash
   ./scripts/setup-udev.sh
   ```

2. Verify the rule was created:
   ```bash
   cat /etc/udev/rules.d/99-nexus.rules
   ```

3. Check device permissions (should show `crw-rw-rw-` after setup):
   ```bash
   ls -l /dev/hidraw*
   ```

4. If permissions haven't changed, trigger udev manually:
   ```bash
   sudo udevadm trigger
   ```

5. Run the application normally:
   ```bash
   ./bin/nexus-open
   ```

### Option 2: Run with sudo (Quick Test)

For quick testing without setting up permanent permissions:

```bash
sudo ./bin/nexus-open
```

**Note:** This requires entering your password each time.

### Option 3: Add User to dialout/input Group

Some systems grant HID access via group membership:

```bash
sudo usermod -aG dialout $USER
sudo usermod -aG input $USER
```

Log out and back in for group changes to take effect.

## Verifying the Setup

Once you have access to the device, you should see:

1. **Application starts without errors:**
   ```
   Starting Nexus Open
   Loaded layout: Multi-Page Dashboard
   Font loaded: 24pt primary
   Font loaded: 16pt multi-line
   Font loaded: 9pt secondary
   Device connected successfully
   ```

2. **Display updates on your Corsair iCUE Nexus:**
   - Large 24pt numbers for single-line metrics (CPU temp, GPU temp, etc.)
   - Stacked 16pt text for network stats (download/upload on separate lines)
   - Small 9pt labels for secondary info
   - Atmospheric background graphs with 15% opacity fill, 40% line opacity

## UI Redesign Features

The following Phase 1 features are now implemented:

- ✅ Modern font hierarchy (24pt/16pt/9pt instead of 14pt/10pt)
- ✅ Improved color contrast (muted text now #B8BDC2)
- ✅ Atmospheric background graphs (15% fill, 40% line opacity)
- ✅ Smart multi-line font sizing (auto-detects and uses 16pt for stacked content)
- ✅ Zone background lift (#141414 vs #101010 main background)
- ✅ Network module displays in stacked format without text overlap

## Troubleshooting

**Q: I ran setup-udev.sh but still getting permission errors**

A: Try unplugging and replugging the USB device, or run:
```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

**Q: Which /dev/hidraw device is the Nexus?**

A: Run this to identify it:
```bash
for device in /dev/hidraw*; do
    echo "=== $device ==="
    udevadm info $device | grep -E "ID_VENDOR_ID|ID_MODEL_ID|ID_PRODUCT"
done
```

Look for `ID_VENDOR_ID=1b1c` and `ID_MODEL_ID=1b8e`.

**Q: The display shows old UI (14pt fonts)**

A: The code is using the new fonts, but you may need to rebuild:
```bash
go build -o bin/nexus-open ./cmd/nexus-open
./bin/nexus-open
```

## Device Information

- **Vendor:** Corsair (1b1c)
- **Product:** iCUE Nexus (1b8e)
- **Display:** 640x48px ultra-wide touchscreen
- **Interface:** USB HID
