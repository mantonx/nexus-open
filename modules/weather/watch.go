package main

import (
	"fmt"
	"path/filepath"

	"github.com/fsnotify/fsnotify"
)

func (m *WeatherModule) watchConfigChanges() {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		fmt.Printf("weather: failed to start config watcher: %v\n", err)
		return
	}
	defer watcher.Close()

	dir := filepath.Dir(m.configPath)
	if err := watcher.Add(dir); err != nil {
		fmt.Printf("weather: failed to watch config dir %s: %v\n", dir, err)
		return
	}

	for {
		select {
		case event, ok := <-watcher.Events:
			if !ok {
				return
			}
			if filepath.Clean(event.Name) != filepath.Clean(m.configPath) {
				continue
			}
			if event.Op&(fsnotify.Write|fsnotify.Create|fsnotify.Rename) != 0 {
				m.loadConfig()
			}
		case err, ok := <-watcher.Errors:
			if !ok {
				return
			}
			fmt.Printf("weather: config watcher error: %v\n", err)
		}
	}
}
