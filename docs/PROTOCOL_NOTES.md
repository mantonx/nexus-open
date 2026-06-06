# iCUE Nexus Protocol Notes

Reverse-engineered protocol details from various sources and our own implementation.

## Device Information

- **Vendor ID**: 0x1b1c (Corsair)
- **Product ID**: 0x1b8e (iCUE Nexus)
- **Display**: 640x48 pixels, touchscreen
- **Color Format**: RGBA32 (but sent as BGR + Alpha to USB)

## USB Communication

### Endpoints

- **Endpoint 2 (OUT)**: Image/display data transfer
- **Feature Reports**: Device control commands
- **Input Reports**: Touch input data

### Display Protocol

Our implementation and NexusTool use **slightly different approaches**:

#### Our Current Implementation (Working)
```
Buffer: 1024*4 = 4096 bytes per chunk
Header:
  [0] = 0x02  // Endpoint
  [1] = 0x05  // Command
  [2] = 0x1F  // 31 decimal
  [3] = 0x00  // 0 for normal chunks, 1 for last chunk
  [4] = chunk_number  // 0-120 (121 chunks total)
  [5] = 0x00
  [6] = 0xF8  // 248 for normal, 192 for last chunk
  [7] = 0x03

Payload: 254-255 pixels per chunk
- Each chunk starts at pixel: chunk_number * 254
- 255 pixels max per chunk (or until end of frame)
- Pixel format: BGR + Alpha (4 bytes)
- Total: 121 chunks, overlapping at boundaries
- Uses buffered writer + flush
```

#### NexusTool Implementation (Alternative)
```
Buffer: 1024 bytes per chunk
Header:
  [0] = 0x02  // Endpoint 2
  [1] = 0x05  // Command
  [2] = 0x40  // Different from ours!
  [3] = 0x00  // 0 for normal, 1 for last block
  [4] = block_number_lo
  [5] = block_number_hi (always 0)
  [6] = payload_length_lo  // Bytes (not pixels!)
  [7] = payload_length_hi

Payload: Up to 1016 bytes per chunk (1024 - 8 header)
- Dynamic chunk size based on remaining data
- No fixed overlap
- Simpler, more efficient
```

**Key Difference**: NexusTool uses `0x40` at byte[2] and variable payload lengths, while we use `0x1F` with fixed pixel counts.

## Feature Report Commands

These use HID Feature Reports (report ID in first byte):

### Brightness Control
```c#
// Set brightness (0-100)
byte[] cmd = [3, 1, brightness];
device.SendFeatureReport(cmd);
```

**We don't have this yet!**

### Animation Control
```c#
// Play animation (1-3), loop = true/false
byte[] cmd = [3, 13, animation, loop ? 1 : 0];
device.SendFeatureReport(cmd);

// Stop animation
byte[] cmd = [3, 15];
device.SendFeatureReport(cmd);
```

**We don't have this yet!**

### Screen Control
```c#
// Blank screen
byte[] cmd = [3, 4];
device.SendFeatureReport(cmd);
```

**We don't have this yet!**

### Device Information
```c#
// Get firmware version
ReadOnlySpan<byte> bytes = device.GetFeatureReport(5, 64);
// Firmware string starts at byte 6, null-terminated
string firmware = ASCII.GetString(bytes[6..pos]);
```

**We don't have this yet!**

## Touch Input Protocol

Touch data comes via input reports:

```c#
// Read touch data
ReadOnlySpan<byte> bytes = device.Read(64);

// Validate header
if (bytes[0] == 0x01 && bytes[1] == 0x02 && bytes[2] == 0x21)
{
    bool touched = bytes[5] != 0;
    int x = bytes[6] + (bytes[7] << 8);  // Little-endian X coordinate

    // X ranges from 0-639
}
```

### Touch Gestures

NexusTool implements gesture detection:

- **Steady Touch**: `diff < 50 pixels` → Touch at specific X
- **Jittery Touch**: `50 < diff < 200` → Uncertain input
- **Swipe Left**: `diff < -200` → Gesture
- **Swipe Right**: `diff > 200` → Gesture

**Our touch.go in nexus/ has basic implementation but not migrated to internal/device yet.**

## Differences from Our Implementation

### What We're Missing

1. **Brightness Control** - Easy to add, just need HID feature reports
2. **Built-in Animations** - Device has 3 firmware animations
3. **Blank Screen Command** - Simple power-saving feature
4. **Firmware Version Query** - Useful for diagnostics
5. **Better Touch Gestures** - We only read raw coordinates

### What We Have That NexusTool Doesn't

1. **Real-time System Monitoring** - CPU/GPU temps, network stats
2. **Weather Integration** - Open-Meteo API with geocoding
3. **Config Hot-Reload** - Changes apply without restart
4. **REST API** - HTTP API for programmatic control
5. **Flutter UI** - Graphical configuration interface
6. **systemd Integration** - Proper Linux service
7. **Comprehensive Packaging** - DEB, AppImage, Flatpak, Snap, AUR

## Implementation Priority

### High Priority (Easy Wins)
- [ ] Add brightness control via Feature Report
- [ ] Add blank screen command
- [ ] Query firmware version on startup
- [ ] Improve touch input with gesture detection

### Medium Priority
- [ ] Add animation playback support
- [ ] Implement touch-based UI navigation
- [ ] Add touch callback system for custom actions

### Low Priority (Nice to Have)
- [ ] Investigate alternate protocol (`0x40` vs `0x1F`)
- [ ] Optimize chunk size/overlap for performance
- [ ] Add touch calibration

## Code Examples

### Adding Brightness Control

In `internal/device/device.go`:
```go
// Add to Device interface
SetBrightness(brightness int) error

// In nexus.go:
func (n *NexusDevice) SetBrightness(brightness int) error {
    if brightness < 0 || brightness > 100 {
        return fmt.Errorf("brightness must be 0-100")
    }

    // HID Feature Report: [3, 1, brightness]
    // This requires adding HID support to gousb or using a different library
    // TODO: Implement HID feature reports
    return nil
}
```

### Adding Firmware Query

```go
func (n *NexusDevice) GetFirmwareVersion() (string, error) {
    // Read Feature Report 5 (64 bytes)
    // Parse firmware string starting at byte 6
    // TODO: Implement HID feature report reading
    return "", nil
}
```

## References

- **NexusTool**: https://github.com/willneedit/NexusTool (C#, HID-based)
- **iCUE-ReverseEngineer**: https://github.com/Aytackydln/iCUE-ReverseEngineer (C#, iCUE emulation)
- **companion-plugin-icue-nexus**: https://github.com/bitfocus/companion-plugin-icue-nexus (TypeScript)

## Notes

- **HID vs Bulk Transfer**: NexusTool uses HID API, we use bulk USB transfers (libusb)
- **Both approaches work**: Different protocols for the same device
- **Feature Reports need HID**: Brightness/animations require HID feature reports, not available in bulk mode
- **Consider hidapi**: Might be worth switching to hidapi library for full feature support

## Testing on Real Hardware

Current status on Arch Linux with actual iCUE Nexus:
- ✅ Display rendering works perfectly
- ✅ Image chunking and transfer verified
- ✅ System monitoring operational
- ✅ Weather fetching functional
- ❌ Touch input not tested yet
- ❌ Brightness control not implemented
- ❌ Animations not implemented
