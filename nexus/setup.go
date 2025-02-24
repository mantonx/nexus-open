// Package nexus provides configuration management and monitoring functionality for the Nexus application.
// It implements a configuration watching system that allows for real-time updates of application settings
// without requiring a restart.
//
// The package includes functionality for:
//   - Monitoring configuration file changes in real-time
//   - Thread-safe access to global configuration settings
//   - Automatic weather updates when location or unit settings change
//   - Configuration comparison and change detection
//
// The package uses mutex locks to ensure thread-safety when accessing shared configuration data
// and implements channels for notifying other components about configuration changes.
//
// Configuration changes are detected and processed at regular intervals defined by configRefreshRate.
// When changes are detected, appropriate update signals are sent through dedicated channels to
// notify dependent components.
package nexus

import (
	"log"
	"nexus-open/nexus/configuration"
	"time"
)

// WatchConfig periodically monitors and reloads the configuration file.
// It runs as a goroutine that checks for configuration changes at regular intervals
// defined by configRefreshRate.
//
// When changes are detected in the configuration:
//   - If location or unit settings change, it triggers an immediate weather update
//   - For any configuration changes, it updates the global configuration and notifies
//     listeners through the update channel
//
// The function uses mutex locks to ensure thread-safe access to shared configuration.
// It will continue running until the program terminates, constantly watching for
// configuration changes.
func WatchConfig() {
	ticker := time.NewTicker(configRefreshRate * time.Second)
	for range ticker.C {
		newConfig, err := configuration.LoadConfig("")
		if err != nil {
			log.Printf("Error loading config: %v", err)
			continue
		}

		configMu.Lock()
		if newConfig.Location != config.Location || newConfig.Unit != config.Unit {
			// Location or unit changed, trigger immediate weather update
			if weatherUpdateCh != nil {
				select {
				case weatherUpdateCh <- struct{}{}:
					log.Printf("Triggered weather update for location: %s", newConfig.Location)
				default:
				}
			}
		}

		// Update config if anything changed
		if configChanged(config, newConfig) {
			config = newConfig
			unit = newConfig.Unit
			location = newConfig.Location
			select {
			case updateCh <- struct{}{}:
			default:
			}
		}
		configMu.Unlock()
	}
}

// GetConfig returns the global Nexus configuration in a thread-safe manner.
// This function uses a read lock to ensure concurrent access safety.
// The returned configuration should not be modified directly.
func GetConfig() *configuration.NexusConfig {
	configMu.RLock()
	defer configMu.RUnlock()
	return config
}

// configChanged compares two NexusConfig configurations and determines if there are any differences
// between them. It checks for changes in Unit, Location, TimeFormat, TextColor, and BackgroundColor settings.
//
// Parameters:
//   - old: A pointer to the original NexusConfig configuration
//   - new: A pointer to the new NexusConfig configuration
//
// Returns:
//   - bool: true if any configuration setting has changed, false if all settings are identical
func configChanged(old, new *configuration.NexusConfig) bool {
	return old.Unit != new.Unit ||
		old.Location != new.Location ||
		old.TimeFormat != new.TimeFormat ||
		old.TextColor != new.TextColor ||
		old.BackgroundColor != new.BackgroundColor
}
