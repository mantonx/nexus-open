package main

import (
    "fmt"
    "github.com/google/gousb"
    "log"
    "image"
    "image/color"
    "image/draw"
    "golang.org/x/image/font"
    "golang.org/x/image/font/basicfont"
    "golang.org/x/image/math/fixed"
)

const (
    vid       = 0x1b1c
    pid       = 0x1b8e
    width     = 640
    height    = 48
    brightness = 2
)

var (
    device    *gousb.Device
    connected bool
)

func main() {
    createHIDMonitor()
}

func createHIDMonitor() {
    ctx := gousb.NewContext()
    defer ctx.Close()

    devices, err := ctx.OpenDevices(func(desc *gousb.DeviceDesc) bool {
        return desc.Vendor == gousb.ID(vid) && desc.Product == gousb.ID(pid)
    })

    if err != nil {
        log.Fatalf("Failed to open devices: %v", err)
    }

    defer func() {
        for _, d := range devices {
            d.Close()
        }
    }()

    if len(devices) > 0 {
        device = devices[0]
        connected = true
        fmt.Println("iCUE Nexus: Device connected.")
        cfg, err := device.Config(1)
        
        if err != nil {
            log.Fatalf("Failed to retrieve config: %v", err)
        }

        defer cfg.Close()

        fmt.Println(device.Product())

        if err := device.SetAutoDetach(true); err != nil {
            log.Fatalf("Unable to reserve device: %v", err)
        }

        testData := make([]byte, width*height*4)
        // Create black background
        for i := 0; i < len(testData); i += 4 {
            testData[i] = 0   // R
            testData[i+1] = 0 // G
            testData[i+2] = 0 // B
            testData[i+3] = 255 // A
        }

        img := image.NewRGBA(image.Rect(0, 0, width, height))
        draw.Draw(img, img.Bounds(), image.Black, image.Point{}, draw.Src)

        d := &font.Drawer{
            Dst:  img,
            Src:  image.NewUniform(color.RGBA{255, 0, 0, 255}),
            Face: basicfont.Face7x13,
            Dot:  fixed.Point26_6{X: fixed.I(10), Y: fixed.I(30)},
        }
        d.DrawString("This is for Chantilly, I'm learning out to work with usb devices.")

        // Copy the image data to testData
        copy(testData, img.Pix)

        if err := setNexusImage(device, testData); err != nil {
            log.Fatalf("Failed to set Nexus image: %v", err)
        }
        
        fmt.Println("Image data sent successfully")
    }
}

func setNexusImage(device *gousb.Device, imageData []byte) error {
    if !connected {
        fmt.Println("iCUE Nexus: not connected.")
        return nil
    }

    fmt.Println("iCUE Nexus: Sending image data...")

    if len(imageData) != width*height*4 {
        return fmt.Errorf("incoming Image Data Length Mismatch")
    }

    // Get device interface and endpoint
    intf, done, err := device.DefaultInterface()
    
    if err != nil {
        return fmt.Errorf("DefaultInterface(): %v", err)
    }

    fmt.Println("Claiming interface...")
    
    defer done()

    ep, err := intf.OutEndpoint(2)
    
    if err != nil {
        return fmt.Errorf("OutEndpoint(2): %v", err)
    }

    data := make([]byte, 1024*4) // Increased buffer size to accommodate header + data
    data[0] = 2
    data[1] = 5
    data[2] = 31
    data[3] = 0
    data[4] = 0
    data[5] = 0
    data[6] = 248
    data[7] = 3

    for i := 0; i <= 120; i++ {
        data[4] = byte(i)
        if i != 120 {
            data[3] = 0
            data[6] = 248
        } else {
            data[3] = 1
            data[6] = 192
        }

        num2 := i * 254
        for num := 0; num < 255 && num2 < 30720; num++ {
            data[8+num*4] = imageData[num2*4+2]   // B
            data[8+num*4+1] = imageData[num2*4+1] // G
            data[8+num*4+2] = imageData[num2*4]   // R
            data[8+num*4+3] = 255                 // A
            num2++
        }

        fmt.Println("Sending data to Nexus200")

        _, err = ep.Write(data)
        
        if err != nil {
            return fmt.Errorf("failed to write data: %v", err)
        }
    }

    return nil
}
