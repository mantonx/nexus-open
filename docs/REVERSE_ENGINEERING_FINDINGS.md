# Reverse Engineering Findings

Research from analyzing three open-source iCUE Nexus implementations.

## Repositories Analyzed

1. **NexusTool** (C#) - https://github.com/willneedit/NexusTool
2. **companion-module-icue-nexus** (TypeScript/Node.js) - https://github.com/bitfocus/companion-module-icue-nexus
3. **iCUE-ReverseEngineer** (C#) - https://github.com/Aytackydln/iCUE-ReverseEngineer

## Key Findings

### 1. Display Protocol (CONFIRMED ACROSS ALL 3 REPOS)

All three implementations use the same core protocol:

```
Packet Structure (1024 bytes):
[0] = 0x02     // Endpoint 2
[1] = 0x05     // Command: Send Image
[2] = 0x1F     // 31 decimal (companion confirms this)
[3] = 0x00/0x01 // Last chunk flag (0=normal, 1=last)
[4] = chunk_id  // 0-120
[5] = 0x00
[6] = 0xF8/0xC0 // 248 for normal, 192 for last
[7] = 0x03

Pixel Data: Starts at byte 8
- 254 pixels per chunk (overlapping)
- BGR + Alpha format
- 4 bytes per pixel
- 121 total chunks (0-120)
```

**Our implementation matches this exactly!** ✅

### 2. Brightness Control (NEW DISCOVERY)

From **companion-module-icue-nexus**, brightness uses specific byte values:

```typescript
const brightnessValues = [0, 4, 12, 16, 64];
// brightness: 0 = off, 1 = 4, 2 = 12, 3 = 16, 4 = 64

Feature Report:
[3, 1, brightnessValue, 1, 120, 0, 192, 3, ...zeros...]
```

**Three different implementations of brightness:**

#### NexusTool (Simple)
```csharp
byte[] cmd = [3, 1, brightness];  // brightness 0-100 directly
device.SendFeatureReport(cmd);
```

#### Companion Module (Mapped)
```typescript
const data = new Uint8Array([
  3, 1,
  [0, 4, 12, 16, 64][Math.floor(brightness)],  // 0-4 maps to specific values
  1, 120, 0, 192, 3, ...zeros...
]);
device.sendFeatureReport(Buffer.from(data));
```

**Analysis**: The Companion module uses a 32-byte feature report with specific brightness mappings, while NexusTool uses a minimal 3-byte report. Both appear to work, suggesting:
- The device accepts multiple formats
- The additional bytes might be optional padding
- Brightness might support both 0-100 range and specific PWM values

### 3. Touch Input (CONFIRMED)

From **NexusTool** - detailed touch protocol:

```
Input Report Structure:
[0] = 0x01     // Report ID
[1] = 0x02     // Report type
[2] = 0x21     // Touch data marker
[5] = touch_state  // 0 = not touched, non-zero = touched
[6] = X_lo     // X coordinate low byte
[7] = X_hi     // X coordinate high byte

X coordinate: 0-639 (full width)
Report size: 64 bytes
Timeout: 2000ms recommended
```

**Touch Gestures** (NexusTool implementation):
- **Steady Touch**: `|diff| < 50 pixels` → Report X position
- **Jittery**: `50 < |diff| < 200` → Uncertain touch
- **Swipe Left**: `diff < -200`
- **Swipe Right**: `diff > 200`

**We have basic touch reading in nexus/touch.go but haven't migrated it yet!**

### 4. Animations (NEW DISCOVERY)

From **NexusTool**:

```csharp
// Play animation (1-3), loop or one-shot
byte[] cmd = [3, 13, animationId, loop ? 1 : 0];
device.SendFeatureReport(cmd);

// Stop animation
byte[] cmd = [3, 15];
device.SendFeatureReport(cmd);
```

The device has **3 built-in firmware animations** that can play in loop or one-shot mode.

### 5. Device Commands Summary

All HID Feature Reports (Report ID = first byte):

```
Command         | Bytes                    | Description
----------------|--------------------------|---------------------------
Brightness      | [3, 1, value]            | value: 0-100
Play Animation  | [3, 13, id, loop]        | id: 1-3, loop: 0/1
Stop Animation  | [3, 15]                  | Stop any playing animation
Blank Screen    | [3, 4]                   | Turn off display
Get Firmware    | Read Report 5            | Returns firmware version at byte 6
```

## Implementation Approaches

### USB Communication Methods

| Repo | Library | Method | Pros | Cons |
|------|---------|--------|------|------|
| **Ours** | libusb (gousb) | Bulk transfer | Fast, direct access | No HID features |
| **NexusTool** | hidapi | HID | Full feature support | Requires hidapi |
| **Companion** | node-hid | HID | Node.js integration | JavaScript only |

**Key Insight**: We use bulk USB transfers which work for display data, but **we can't access HID Feature Reports** for brightness, animations, etc., without adding HID support.

## What We Can Add Immediately

### 1. Brightness Control (Requires HID Support)

Options:
- **A. Add go-hid library**: Switch from gousb to hid library
- **B. Use both**: Keep gousb for display, add hid for features
- **C. CGO to hidapi**: Direct C bindings to hidapi

Recommended: **Option B** - Keep our working display code, add hid library for features.

### 2. Touch Gestures (Easy - Just Code)

We have touch reading, just need to add gesture detection:

```go
type TouchGesture int

const (
    TouchNone TouchGesture = iota
    TouchSteady  // Tap at position
    TouchJittery // Uncertain
    TouchSwipeLeft
    TouchSwipeRight
)

func (t *TouchHandler) DetectGesture(first, last int) TouchGesture {
    diff := last - first
    absDiff := abs(diff)

    if absDiff < 50 {
        return TouchSteady
    } else if absDiff < 200 {
        return TouchJittery
    } else if diff < 0 {
        return TouchSwipeLeft
    } else {
        return TouchSwipeRight
    }
}
```

### 3. Device Information

Can be queried via HID Feature Report 5 (needs HID support).

## Architecture Recommendations

### Option A: Keep Current Architecture (Display Only)

**Pros:**
- Working now
- No breaking changes
- Simple

**Cons:**
- Missing brightness, animations, advanced features
- Not feature-complete

### Option B: Add HID Support (Recommended)

**Pros:**
- Full feature access
- Best of both worlds
- Professional implementation

**Cons:**
- Need to add dependency
- Slightly more complex

### Option C: Pure HID Implementation

**Pros:**
- Unified communication
- All features available
- Simpler architecture

**Cons:**
- Need to rewrite display code
- Risk breaking working implementation
- NexusTool shows HID works but uses different protocol variant

## Recommended Next Steps

### Immediate (No new dependencies)
1. ✅ Document findings (this file)
2. ⬜ Migrate touch input to internal/device
3. ⬜ Add gesture detection
4. ⬜ Add touch event callbacks

### Short Term (Add HID library)
1. ⬜ Evaluate Go HID libraries (karalabe/hid, sstallion/go-hid)
2. ⬜ Add HID device interface alongside USB
3. ⬜ Implement brightness control
4. ⬜ Implement animation commands
5. ⬜ Implement firmware query

### Long Term (Polish)
1. ⬜ Add brightness slider to Flutter UI
2. ⬜ Add touch-based navigation
3. ⬜ Add animation preview/selection
4. ⬜ Power management integration

## Code Examples

### Adding HID Support (Pseudo-code)

```go
// internal/device/hid_device.go
package device

import "github.com/karalabe/hid"

type HIDDevice struct {
    device *hid.Device
    vendorID  uint16
    productID uint16
}

func (h *HIDDevice) SetBrightness(brightness int) error {
    if brightness < 0 || brightness > 100 {
        return ErrInvalidBrightness
    }

    // Use companion module's approach with mapped values
    brightnessMap := []byte{0, 4, 12, 16, 64}
    level := brightness / 25  // Map 0-100 to 0-4
    if level > 4 {
        level = 4
    }

    data := make([]byte, 32)
    data[0] = 3  // Report ID
    data[1] = 1  // Brightness command
    data[2] = brightnessMap[level]
    data[3] = 1
    data[4] = 120
    data[5] = 0
    data[6] = 192
    data[7] = 3
    // Rest is zeros

    _, err := h.device.SendFeatureReport(data)
    return err
}

func (h *HIDDevice) PlayAnimation(id int, loop bool) error {
    if id < 1 || id > 3 {
        return ErrInvalidAnimation
    }

    data := []byte{3, 13, byte(id), 0}
    if loop {
        data[3] = 1
    }

    _, err := h.device.SendFeatureReport(data)
    return err
}

func (h *HIDDevice) GetFirmwareVersion() (string, error) {
    data := make([]byte, 64)
    n, err := h.device.GetFeatureReport(data)
    if err != nil {
        return "", err
    }

    // Find null terminator starting at byte 6
    end := 6
    for end < n && data[end] != 0 {
        end++
    }

    return string(data[6:end]), nil
}
```

## Testing Notes

**Tested on**: Arch Linux with actual iCUE Nexus hardware
- ✅ Display protocol works perfectly
- ✅ BGR color format confirmed
- ✅ 121-chunk protocol validated
- ✅ Buffered writer necessary
- ⬜ Touch input not tested yet
- ⬜ HID features not accessible (using bulk USB)

## Conclusion

Our implementation is **solid and working** for display functionality. The protocol matches all three reference implementations perfectly.

To be **feature-complete**, we should add HID support for:
- Brightness control
- Built-in animations
- Firmware queries
- Better touch handling

This can be done **incrementally** without breaking existing functionality.

## References

### NexusTool (C#)
- **URL**: https://github.com/willneedit/NexusTool
- **Key Files**:
  - `NexusTool/Nexus.cs` - Main device class
  - Uses hidapi library
  - Simple, clean implementation

### Companion Module (TypeScript)
- **URL**: https://github.com/bitfocus/companion-module-icue-nexus
- **Key Files**:
  - `src/nexus.ts` - Device communication
  - Uses node-hid library
  - Brightness mapping discovery

### iCUE-ReverseEngineer (C#)
- **URL**: https://github.com/Aytackydln/iCUE-ReverseEngineer
- **Purpose**: iCUE protocol emulation
- Less relevant for Nexus-specific features
