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

// WithPluginsDir sets the directory where exec: plugin binaries are found.
// Defaults to a sibling plugins/ directory next to the running executable,
// or ~/.local/lib/nexus-open/plugins when running from a system install.
func WithPluginsDir(path string) Option {
	return func(a *App) {
		a.pluginsDir = path
	}
}
