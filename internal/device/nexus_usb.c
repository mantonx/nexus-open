#include "nexus_usb.h"

#include <libusb-1.0/libusb.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

static libusb_context *nexus_ctx = NULL;

struct NexusHandle {
	libusb_device_handle *display; // interface 0: EP 0x02 OUT + EP 0x81 IN
	char manufacturer[256];
	char product[256];
};

int nexus_init(void) {
	if (nexus_ctx != NULL)
		return 0;
	return libusb_init(&nexus_ctx);
}

void nexus_exit(void) {
	if (nexus_ctx != NULL) {
		libusb_exit(nexus_ctx);
		nexus_ctx = NULL;
	}
}

NexusHandle *nexus_open(uint16_t vid, uint16_t pid, char *errbuf, int errbuf_len) {
	libusb_device **devs = NULL;
	ssize_t cnt = libusb_get_device_list(nexus_ctx, &devs);
	if (cnt < 0) {
		snprintf(errbuf, errbuf_len, "libusb_get_device_list: %s", libusb_error_name((int)cnt));
		return NULL;
	}

	libusb_device *target = NULL;
	struct libusb_device_descriptor desc;
	for (ssize_t i = 0; i < cnt; i++) {
		if (libusb_get_device_descriptor(devs[i], &desc) < 0)
			continue;
		if (desc.idVendor == vid && desc.idProduct == pid) {
			target = devs[i];
			break;
		}
	}
	if (!target) {
		libusb_free_device_list(devs, 1);
		snprintf(errbuf, errbuf_len, "device %04x:%04x not found", vid, pid);
		return NULL;
	}

	libusb_device_handle *dh = NULL;
	int r = libusb_open(target, &dh);
	if (r < 0) {
		snprintf(errbuf, errbuf_len, "libusb_open: %s", libusb_error_name(r));
		libusb_free_device_list(devs, 1);
		return NULL;
	}

	NexusHandle *nh = (NexusHandle *)calloc(1, sizeof(NexusHandle));
	nh->display = dh;

	libusb_get_string_descriptor_ascii(dh, desc.iManufacturer,
		(unsigned char *)nh->manufacturer, sizeof(nh->manufacturer));
	libusb_get_string_descriptor_ascii(dh, desc.iProduct,
		(unsigned char *)nh->product, sizeof(nh->product));

	if (libusb_kernel_driver_active(dh, 0) == 1)
		libusb_detach_kernel_driver(dh, 0);

	r = libusb_claim_interface(dh, 0);
	if (r < 0) {
		snprintf(errbuf, errbuf_len, "claim interface 0: %s", libusb_error_name(r));
		libusb_close(dh);
		free(nh);
		libusb_free_device_list(devs, 1);
		return NULL;
	}

	// Prime EP 0x81 IN so the device accepts EP 0x02 OUT writes.
	unsigned char prime_buf[512];
	int transferred = 0;
	libusb_interrupt_transfer(dh, 0x81, prime_buf, sizeof(prime_buf), &transferred, 100);

	libusb_free_device_list(devs, 1);
	return nh;
}

void nexus_close(NexusHandle *nh) {
	if (!nh) return;
	if (nh->display) {
		libusb_release_interface(nh->display, 0);
		libusb_close(nh->display);
	}
	free(nh);
}

int nexus_write_frame(NexusHandle *nh, const unsigned char *data, int length) {
	int transferred = 0;
	int r = libusb_interrupt_transfer(nh->display, 0x02,
		(unsigned char *)data, length, &transferred, 1000);
	if (r < 0) return r;
	return transferred;
}

int nexus_read_touch(NexusHandle *nh, unsigned char *buf, int length, unsigned int timeout_ms) {
	int transferred = 0;
	int r = libusb_interrupt_transfer(nh->display, 0x81,
		buf, length, &transferred, timeout_ms);
	if (r < 0) return r;
	return transferred;
}

int nexus_set_brightness(NexusHandle *nh, uint8_t value) {
	unsigned char data[3] = {3, 1, value};
	int r = libusb_control_transfer(nh->display,
		LIBUSB_REQUEST_TYPE_CLASS | LIBUSB_RECIPIENT_INTERFACE | LIBUSB_ENDPOINT_OUT,
		0x09,          // HID SET_REPORT
		(3 << 8) | 3, // wValue: type=Feature(3), report_id=3
		0,             // wIndex: interface 0
		data, sizeof(data),
		1000);
	return r < 0 ? r : 0;
}

const char *nexus_manufacturer(NexusHandle *nh) { return nh->manufacturer; }
const char *nexus_product(NexusHandle *nh)      { return nh->product; }
