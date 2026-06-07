#pragma once

#include <stdint.h>

// NexusHandle holds the libusb device handle for interface 0, which carries
// both frame writes (EP 0x02 OUT) and touch reads (EP 0x81 IN).
typedef struct NexusHandle NexusHandle;

int          nexus_init(void);
void         nexus_exit(void);

// nexus_open opens the device by VID/PID.
// Returns NULL on failure; writes a human-readable message into errbuf.
NexusHandle *nexus_open(uint16_t vid, uint16_t pid, char *errbuf, int errbuf_len);
void         nexus_close(NexusHandle *nh);

// nexus_write_frame sends one 1024-byte packet to EP 0x02 OUT.
// Returns bytes transferred, or negative libusb error code.
int          nexus_write_frame(NexusHandle *nh, const unsigned char *data, int length);

// nexus_read_touch reads one touch packet from EP 0x81 IN.
// Returns bytes read, or negative libusb error code.
int          nexus_read_touch(NexusHandle *nh, unsigned char *buf, int length, unsigned int timeout_ms);

// nexus_set_brightness sends a HID SET_REPORT feature report (report ID 3).
// Returns 0 on success, negative libusb error code on failure.
int          nexus_set_brightness(NexusHandle *nh, uint8_t value);

const char  *nexus_manufacturer(NexusHandle *nh);
const char  *nexus_product(NexusHandle *nh);
