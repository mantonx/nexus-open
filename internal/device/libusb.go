package device

/*
#cgo pkg-config: libusb-1.0
#include <libusb-1.0/libusb.h>
#include "nexus_usb.h"
*/
import "C"
import (
	"fmt"
	"sync"
	"unsafe"
)

func init() {
	if rc := C.nexus_init(); rc != 0 {
		panic(fmt.Sprintf("libusb_init failed: %d", rc))
	}
}

// usbHandle wraps the C NexusHandle pointer.
type usbHandle struct {
	mu     sync.Mutex
	ptr    *C.NexusHandle
	closed bool
}

// usbOpen finds and opens the Nexus device by VID/PID.
func usbOpen(vid, pid uint16) (*usbHandle, string, string, error) {
	buf := make([]byte, 256)
	h := C.nexus_open(
		C.uint16_t(vid),
		C.uint16_t(pid),
		(*C.char)(unsafe.Pointer(&buf[0])),
		C.int(len(buf)),
	)
	if h == nil {
		return nil, "", "", fmt.Errorf("%s", C.GoString((*C.char)(unsafe.Pointer(&buf[0]))))
	}
	mfr := C.GoString(C.nexus_manufacturer(h))
	prod := C.GoString(C.nexus_product(h))
	return &usbHandle{ptr: h}, mfr, prod, nil
}

func (h *usbHandle) close() {
	if h == nil {
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	if !h.closed {
		h.closed = true
		C.nexus_close(h.ptr)
		h.ptr = nil
	}
}

// writeFrame sends a 1024-byte frame packet to EP 0x02 OUT.
func (h *usbHandle) writeFrame(pkt []byte) error {
	if len(pkt) != 1024 {
		return fmt.Errorf("frame packet must be 1024 bytes, got %d", len(pkt))
	}
	rc := C.nexus_write_frame(h.ptr, (*C.uchar)(unsafe.Pointer(&pkt[0])), C.int(len(pkt)))
	if rc < 0 {
		return fmt.Errorf("USB write: %s", C.GoString(C.libusb_error_name(C.int(rc))))
	}
	return nil
}

// ReadTouch satisfies touch.TouchDevice.
// Uses a 200ms timeout so the goroutine unblocks promptly when the handle is
// closed. LIBUSB_ERROR_TIMEOUT (-7) is returned as (0, nil) — no data, not an error.
func (h *usbHandle) ReadTouch(buf []byte, _ uint) (int, error) {
	h.mu.Lock()
	if h.closed {
		h.mu.Unlock()
		return 0, fmt.Errorf("USB read: handle closed")
	}
	ptr := h.ptr
	h.mu.Unlock()

	const internalTimeoutMs = 200
	rc := C.nexus_read_touch(ptr,
		(*C.uchar)(unsafe.Pointer(&buf[0])),
		C.int(len(buf)),
		C.uint(internalTimeoutMs),
	)
	if rc == -7 { // LIBUSB_ERROR_TIMEOUT
		return 0, nil
	}
	if rc < 0 {
		return 0, fmt.Errorf("USB read: %s", C.GoString(C.libusb_error_name(C.int(rc))))
	}
	return int(rc), nil
}

// setBrightness sends the brightness command (0–100) via SET_REPORT.
func (h *usbHandle) setBrightness(value int) error {
	rc := C.nexus_set_brightness(h.ptr, C.uint8_t(value))
	if rc < 0 {
		return fmt.Errorf("USB brightness: %s", C.GoString(C.libusb_error_name(C.int(rc))))
	}
	return nil
}
