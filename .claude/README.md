# Nexus Open Development Notes

## Hot Reload Development

**IMPORTANT**: This project uses Air for hot reloading during development.

### Starting the Development Server

```bash
./dev.sh
```

Or directly with air:

```bash
~/go/bin/air
```

### What This Does

- Watches for file changes in `*.go`, `configs/`, `internal/`, etc.
- Automatically rebuilds and restarts the app on changes
- Logs to `tmp/main.log`
- Binary output to `tmp/main`

### DO NOT manually build and restart during development

Instead of:
```bash
# DON'T DO THIS during dev:
go build -o nexus-open ./cmd/nexus-open
pkill nexus-open
./nexus-open
```

Just let air handle it automatically when you save files.

### Air Configuration

See `.air.toml` in the project root for configuration.

### When to Manual Build

Only build manually for:
- Production builds
- Testing without air
- Debugging specific issues

## Module Development

When modifying modules (cpu-temp, gpu-temp, network, weather):
1. Make changes to `modules/*/main.go`
2. Rebuild the specific module:
   ```bash
   cd modules/cpu-temp && go build -o cpu-temp .
   ```
3. Air will detect the binary change and restart the app

## Project Structure

- `internal/zone/` - Zone system (layout, rendering, config, sampling)
- `internal/settings/` - Global UI configuration
- `internal/api/` - REST API with zone config endpoints
- `pkg/module/` - Module interface and types
- `modules/` - Plugin modules (cpu-temp, gpu-temp, network, weather)
