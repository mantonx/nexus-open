package app

import "log/slog"

// Option is a functional option for configuring the App.
type Option func(*App)

// WithLogger sets a custom logger for the application.
func WithLogger(logger *slog.Logger) Option {
	return func(a *App) {
		a.logger = logger
	}
}

// WithConfigPath sets the configuration file path.
func WithConfigPath(path string) Option {
	return func(a *App) {
		a.configPath = path
	}
}

// WithAPIPort sets the API server port.
func WithAPIPort(port int) Option {
	return func(a *App) {
		a.apiPort = port
	}
}

// WithLayoutPath sets the layout config file path.
func WithLayoutPath(path string) Option {
	return func(a *App) {
		a.layoutPath = path
	}
}
