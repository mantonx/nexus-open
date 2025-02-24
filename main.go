package main

import (
	_ "embed"
	"nexus-open/nexus"
)

// //go:embed icon.ico
// var iconBytes []byte

// func onReady() {
// 	systray.SetIcon(iconBytes)
// 	systray.SetTitle("Nexus Open")
// 	systray.SetTooltip("Nexus Open Status")

// 	quitOpenNexus := systray.AddMenuItem("Quit", "Quit the app")

// 	go func() {
// 		<-quitOpenNexus.ClickedCh
// 		systray.Quit()
// 	}()

// 	nexus.StartNexus()
// }

// func onExit() {
// 	nexus.StopNexus()
// }

func main() {
	nexus.StartNexus()
	// systray.Run(onReady, onExit)
	// Create an instance of the app structure
	// app := NewApp()

	// Start Nexus in a separate goroutine
	// nexus.StartNexus()

	// // Create application with options
	// err := wails.Run(&options.App{
	// 	Title:         "Nexus Open",
	// 	Width:         1024,
	// 	Height:        768,
	// 	DisableResize: true,
	// 	AssetServer: &assetserver.Options{
	// 		Assets: assets,
	// 	},
	// 	Frameless:        false,
	// 	BackgroundColour: &options.RGBA{R: 255, G: 255, B: 0, A: 1}, // Yellow
	// 	OnStartup:        app.startup,
	// 	Bind: []interface{}{
	// 		app,
	// 	},
	// })

	// if err != nil {
	// 	println("Error:", err.Error())
	// }
}
