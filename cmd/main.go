package main

import (
    "fmt"
    "github.com/google/gousb"
    "log"
)

func main() {
    ctx := gousb.NewContext()
    defer ctx.Close()

    ctx.Debug(4) // Enable debug logs

    // List all devices
    devices, err := ctx.OpenDevices(func(desc *gousb.DeviceDesc) bool {
        return true
    })
    
    if err != nil {
        log.Fatalf("Failed to open devices: %v", err)
    }
    
    defer func() {
        for _, d := range devices {
            d.Close()
        }
    }()

    // Find the touchscreen device
    var touchscreen *gousb.Device
    
    for _, d := range devices {
        if d.Desc.Vendor == gousb.ID(0x1b1c) && d.Desc.Product == gousb.ID(0x1b8e) { // Replace with actual VendorID and ProductID
            touchscreen = d
            break
        }
    }

    if touchscreen == nil {
        log.Fatalf("Touchscreen device not found")
    }

    if err := touchscreen.SetAutoDetach(true); err != nil {
        log.Fatalf("Failed to set auto detach: %v", err)
    }

    // Claim interface 0
    intf, done, err := touchscreen.DefaultInterface()
    if err != nil {
        log.Fatalf("Failed to claim interface: %v", err)
    }
    defer done()

    // Open endpoint for writing
    ep, err := intf.OutEndpoint(2)
    if err != nil {
        log.Fatalf("Failed to get endpoint: %v", err)
    }

    fmt.Println(ep) 

    // Create a test image (simple black and white pattern)
    testImage := make([]byte, 1024)  // Adjust size based on display requirements
    for i := range testImage {
        if i%2 == 0 {
            testImage[i] = 0xFF
        }
    }

    // // Send the test image
    _, err = ep.Write(testImage)
    if err != nil {
        log.Fatalf("Failed to write to endpoint: %v", err)
    }
    ctx.Close()

    fmt.Println("Test image sent to the touchscreen display")
}