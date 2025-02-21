package main

import (
	"context"
	"fmt"
)

type Config struct {
	HexColor string
	Unit     string
}

// App struct
type App struct {
	ctx context.Context
}

// NewApp creates a new App application struct
func NewApp() *App {
	return &App{}
}

// startup is called when the app starts. The context is saved
// so we can call the runtime methods
func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
}

// SetColor demonstrates setting the window color and logging it
func (a *App) UpdateConfig(config Config) {
	hexColor := config.HexColor
	unit := config.Unit
	fmt.Println("Setting color to:", hexColor)
	fmt.Println("Setting unit to:", unit)
}
