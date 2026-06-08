# Nexus Open - Makefile
# Standardized build system for all targets

.PHONY: help setup doctor build build-debug build-release build-ui build-plugins build-all test test-race coverage clean clean-ui install uninstall run run-tray dev dev-backend dev-ui deb appimage rpm generate-api models all changelog

# Configuration
APP_NAME := nexus-open
VERSION := $(shell git describe --tags --match 'v*' --always --dirty 2>/dev/null | sed 's/^v//' || echo "0.0.0-dev")
COMMIT  := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_TIME := $(shell date -u '+%Y-%m-%d_%H:%M:%S')

# Directories
BIN_DIR := bin
BUILD_DIR := build
DIST_DIR := dist
CMD_DIR := cmd/nexus-open
UI_DIR := ui
UI_BUILD_DIR := $(UI_DIR)/build/linux/x64/release/bundle

# Tool versions — keep in sync with .github/workflows/ci.yml
SQLC_VERSION         := v1.31.1
GOVULNCHECK_VERSION  := v1.3.0
GOLANGCI_VERSION     := v2.12.2
AIR_VERSION          := v1.63.1
GOPENAPI_VERSION     := v0.32.3

# Build flags
LDFLAGS := -X main.version=$(VERSION) -X main.commit=$(COMMIT) -X main.buildTime=$(BUILD_TIME)
GOFLAGS  := -trimpath
GO_BUILD := CGO_ENABLED=1 go build

# Default target
help:
	@echo "Nexus Open - Build System"
	@echo ""
	@echo "UI Testing:"
	@echo "  make screenshot-tour - Navigate all tabs and capture screenshots"
	@echo ""
	@echo "Development:"
	@echo "  make setup         - Install dev tool dependencies (air, overmind)"
	@echo "  make doctor        - Check runtime health (user) and toolchain (dev)"
	@echo "  make build         - Build Go backend only (with debug info)"
	@echo "  make build-ui      - Build Flutter UI only"
	@echo "  make build-plugins - Build all external plugins"
	@echo "  make build-all     - Build backend, UI, and all plugins"
	@echo "  make build-release - Build optimized release binary (stripped)"
	@echo "  make run           - Build and run Go backend only"
	@echo "  make run-tray      - Build and run bundled app with system tray"
	@echo "  make dev-backend   - Go hot-reload via air (rebuilds daemon+plugins on save)"
	@echo "  make dev-ui        - Flutter hot-reload (run alongside dev-backend)"
	@echo ""
	@echo "Testing:"
	@echo "  make test          - Run all tests"
	@echo "  make test-race     - Run tests with race detector"
	@echo "  make coverage      - Generate test coverage report"
	@echo ""
	@echo "Packaging:"
	@echo "  make deb           - Build DEB package"
	@echo "  make appimage      - Build AppImage"
	@echo "  make rpm           - Build RPM package"
	@echo "  make generate-api  - Regenerate api/openapi.yaml from annotations"
	@echo "  make generate-store - Regenerate internal/store/db/ from schema + queries"
	@echo "  make models        - Generate freezed/json_serializable Dart models"
	@echo "  make all           - Build all packages"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean         - Remove build artifacts"
	@echo "  make install       - Build and install backend + UI, restart service"
	@echo "  make uninstall     - Stop service and remove installed binary"
	@echo ""
	@echo "Version: $(VERSION) (commit: $(COMMIT))"

# Create directories
$(BIN_DIR):
	@mkdir -p $(BIN_DIR)

$(DIST_DIR):
	@mkdir -p $(DIST_DIR)

# Build development binary (with debug info)
build: $(BIN_DIR)
	@echo "Building $(APP_NAME) (development)..."
	@$(GO_BUILD) $(GOFLAGS) -ldflags "$(LDFLAGS)" -o $(BIN_DIR)/$(APP_NAME) ./$(CMD_DIR)
	@echo "✓ Built: $(BIN_DIR)/$(APP_NAME)"

# Build with debug symbols (alias)
build-debug: build

# Build optimized release binary
build-release: $(BIN_DIR)
	@echo "Building $(APP_NAME) (release)..."
	@$(GO_BUILD) $(GOFLAGS) -ldflags "$(LDFLAGS) -s -w" -o $(BIN_DIR)/$(APP_NAME) ./$(CMD_DIR)
	@strip $(BIN_DIR)/$(APP_NAME) 2>/dev/null || true
	@echo "✓ Built and stripped: $(BIN_DIR)/$(APP_NAME)"
	@ls -lh $(BIN_DIR)/$(APP_NAME)

# Build Flutter UI
build-ui: $(BIN_DIR)
	@echo "Building Flutter UI..."
	@if ! command -v flutter > /dev/null; then \
		echo "Error: Flutter not found. Install from https://flutter.dev"; \
		exit 1; \
	fi
	@cd $(UI_DIR) && flutter build linux --release --dart-define=APP_VERSION=$(VERSION)
	@echo "Copying Flutter bundle to bin directory..."
	@rm -rf $(BIN_DIR)/nexus-open-ui-bundle
	@cp -r $(UI_BUILD_DIR) $(BIN_DIR)/nexus-open-ui-bundle
	@ln -sf nexus-open-ui-bundle/ui $(BIN_DIR)/nexus-open-ui
	@echo "✓ Flutter UI built: $(BIN_DIR)/nexus-open-ui"

# Build all external plugins
build-plugins:
	@echo "Building external plugins..."
	@for mod in cpu-temp gpu-temp network weather cpu-load gpu-load; do \
		if [ -d plugins/$$mod ]; then \
			echo "  → plugins/$$mod"; \
			(cd plugins/$$mod && go build -o $$mod .) || exit 1; \
		fi; \
	done
	@echo "✓ Plugins built"

# Build backend, UI, and all plugins
build-all: build build-ui build-plugins
	@echo "✓ Complete build finished!"
	@ls -lh $(BIN_DIR)/

# Run the application (backend only)
run: build
	@echo "Running $(APP_NAME)..."
	@$(BIN_DIR)/$(APP_NAME)

# Run bundled application with system tray
run-tray: build-all
	@echo "Running $(APP_NAME) with system tray and UI..."
	@$(BIN_DIR)/$(APP_NAME) --tray

# Navigate all UI tabs and capture screenshots via Dart VM service.
# Requires: Go backend running (make run) and DISPLAY set.
screenshot-tour:
	@./scripts/ui-tour.sh

# Install dev tool dependencies
setup:
	@echo "Installing air (Go live reload)..."
	@go install github.com/air-verse/air@$(AIR_VERSION)
	@echo "Checking for overmind (process manager)..."
	@if ! command -v overmind > /dev/null; then \
		echo "overmind not found. Install it with:"; \
		echo "  Arch:         sudo pacman -S overmind"; \
		echo "  Debian/Ubuntu: sudo apt install overmind"; \
		echo "  macOS:        brew install overmind"; \
		echo "  Go:           go install github.com/DarthSim/overmind/v2@latest"; \
	else \
		echo "overmind already installed: $$(overmind --version)"; \
	fi
	@echo "✓ Setup complete. Run 'make dev-backend' and 'make dev-ui' to start."

# Check runtime health (end-user) and dev toolchain (contributor).
# Exits 1 if any required check fails, so it can be used in scripts.
doctor:
	@fail=0; \
	ok()   { printf '  \033[32m✓\033[0m %s\n' "$$*"; }; \
	warn() { printf '  \033[33m!\033[0m %s\n' "$$*"; }; \
	bad()  { printf '  \033[31m✗\033[0m %s\n' "$$*"; fail=1; }; \
	\
	echo "── Runtime ─────────────────────────────────────────"; \
	\
	if systemctl --user is-active --quiet nexus-open.service 2>/dev/null; then \
		ok "daemon running (systemd)"; \
	elif pgrep -x nexus-open > /dev/null 2>&1; then \
		ok "daemon running (standalone)"; \
	else \
		bad "daemon not running — run 'make install' then start the service, or 'make dev-backend'"; \
	fi; \
	\
	if curl -sf -H "X-Nexus-Token: $$(cat ~/.config/nexus-open/token 2>/dev/null)" \
		http://localhost:1985/api/health > /dev/null 2>&1; then \
		ok "API reachable at localhost:1985"; \
	else \
		bad "API not reachable — is the daemon running?"; \
	fi; \
	\
	if [ -f ~/.config/nexus-open/token ]; then \
		ok "capability token present"; \
	else \
		bad "token missing at ~/.config/nexus-open/token — start the daemon once to generate it"; \
	fi; \
	\
	if lsusb 2>/dev/null | grep -q "1b1c:1b8e"; then \
		ok "Nexus device detected (USB 1b1c:1b8e)"; \
	else \
		warn "Nexus device not detected — not connected, or using mock mode"; \
	fi; \
	\
	if ls /etc/udev/rules.d/99-corsair-nexus.rules /usr/lib/udev/rules.d/99-corsair-nexus.rules > /dev/null 2>&1; then \
		ok "udev rules installed"; \
	else \
		bad "udev rules missing — run: sudo nexus-open --setup-udev"; \
	fi; \
	\
	if groups | grep -qE '\bplugdev\b'; then \
		ok "user in plugdev group"; \
	else \
		warn "user not in plugdev group (may still work via udev TAG+=\"uaccess\")"; \
	fi; \
	\
	echo ""; \
	echo "── Dev toolchain ───────────────────────────────────"; \
	\
	if command -v go > /dev/null; then \
		ok "go $$(go version | awk '{print $$3}')"; \
	else \
		bad "go not found"; \
	fi; \
	\
	if command -v flutter > /dev/null; then \
		ok "flutter $$(flutter --version --machine 2>/dev/null | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("frameworkVersion","?"))' 2>/dev/null || flutter --version 2>&1 | head -1)"; \
	else \
		bad "flutter not found — see https://flutter.dev/docs/get-started/install/linux"; \
	fi; \
	\
	AIR=$$(command -v air 2>/dev/null || echo ~/go/bin/air); \
	if [ -x "$$AIR" ]; then \
		ok "air $$($$AIR -v 2>&1 | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' | head -1)"; \
	else \
		bad "air not found — run 'make setup'"; \
	fi; \
	\
	if command -v overmind > /dev/null; then \
		ok "overmind $$(overmind --version 2>&1)"; \
	else \
		bad "overmind not found — run 'make setup' for install instructions"; \
	fi; \
	\
	SQLC=$$(command -v sqlc 2>/dev/null || echo ~/go/bin/sqlc); \
	if [ -x "$$SQLC" ]; then \
		SQLC_INST=$$($$SQLC version 2>&1 | head -1); \
		ok "sqlc $$SQLC_INST"; \
	else \
		warn "sqlc not found (only needed to regenerate DB queries) — go install github.com/sqlc-dev/sqlc/cmd/sqlc@$(SQLC_VERSION)"; \
	fi; \
	\
	if command -v golangci-lint > /dev/null; then \
		ok "golangci-lint $$(golangci-lint --version 2>&1 | head -1)"; \
	else \
		warn "golangci-lint not found (only needed for lint) — see https://golangci-lint.run/usage/install/"; \
	fi; \
	\
	echo ""; \
	if [ $$fail -eq 0 ]; then \
		echo "All checks passed."; \
	else \
		echo "Some checks failed — see above."; \
		exit 1; \
	fi

# Development mode with live reload (requires github.com/air-verse/air)
# Rebuilds and restarts the Go daemon + plugins on any .go file change.
# The installed plugins in ~/.local/share/nexus-open/plugins are used at runtime;
# run 'make install' once first so the layout config and plugins are in place.
dev-backend:
	@AIR=$$(command -v air 2>/dev/null || echo ~/go/bin/air); \
	if [ ! -x "$$AIR" ]; then \
		echo "Error: 'air' not found. Install with: go install github.com/air-verse/air@$(AIR_VERSION)"; \
		exit 1; \
	fi; \
	NEXUS_MOCK_DEVICE=0 NEXUS_DEBUG=1 "$$AIR"

# Flutter hot-reload UI (runs flutter run in debug mode).
# The backend must already be running (make run or make dev-backend).
# Token is read from ~/.config/nexus-open/token automatically.
dev-ui:
	@if [ ! -f ~/.config/nexus-open/token ]; then \
		echo "Error: token not found at ~/.config/nexus-open/token — is the backend running?"; \
		exit 1; \
	fi
	@cd ui && flutter run -d linux

# Alias: run both backend watcher and UI hot-reload in split terminals.
# Usage: open two terminals and run 'make dev-backend' and 'make dev-ui'.
dev: dev-backend

# Run all tests
test:
	@echo "Running tests..."
	@go test -v ./...

# Run tests with race detector
test-race:
	@echo "Running tests with race detector..."
	@go test -race -v ./...

# Generate coverage report
coverage:
	@echo "Generating coverage report..."
	@go test -coverprofile=coverage.out ./...
	@go tool cover -html=coverage.out -o coverage.html
	@echo "✓ Coverage report: coverage.html"
	@go tool cover -func=coverage.out | grep total:

# Build DEB package
deb: $(DIST_DIR)
	@echo "Building DEB package..."
	@bash scripts/build-deb.sh

# Build AppImage
appimage: $(DIST_DIR)
	@echo "Building AppImage..."
	@bash scripts/build-appimage.sh

# Build RPM package
rpm: $(DIST_DIR)
	@echo "Building RPM package..."
	@bash scripts/build-rpm.sh

# Regenerate OpenAPI spec from source annotations
generate-api:
	@echo "Regenerating OpenAPI spec..."
	@if command -v go-openapi > /dev/null || [ -f ~/go/bin/go-openapi ]; then \
		GO_OPENAPI=$$(command -v go-openapi 2>/dev/null || echo ~/go/bin/go-openapi); \
		$$GO_OPENAPI -dir cmd/nexus-open,internal/api,internal/settings -output api/openapi.yaml; \
		sed -i 's|http://localhost:8080|http://localhost:1985|g' api/openapi.yaml; \
		echo "✓ Spec written to api/openapi.yaml"; \
	else \
		echo "Error: go-openapi not found. Install with: go install github.com/go-openapi/cmd/go-openapi@$(GOPENAPI_VERSION)"; \
		exit 1; \
	fi

# Generate freezed/json_serializable Dart models from api_models.dart
models:
	@echo "Generating Dart models..."
	@cd $(UI_DIR) && dart run build_runner build --delete-conflicting-outputs
	@echo "✓ Models generated"

# Build all packages
all: deb appimage rpm
	@echo "✓ All packages built successfully!"
	@ls -lh $(DIST_DIR)/

# Install to user service location and restart the systemd service.
# The service runs from ~/.local/bin/nexus-open and the UI from
# ~/.local/share/nexus-open/ui.real -- both must be updated together.
INSTALL_BIN    := $(HOME)/.local/bin/$(APP_NAME)
INSTALL_DATA   := $(HOME)/.local/share/$(APP_NAME)
SYSTEMD_UNIT   := app-nexus\x2dopen\x2dautostart@autostart.service

install: build-release build-ui build-plugins
	@echo "Stopping service..."
	@systemctl --user stop "$(SYSTEMD_UNIT)" 2>/dev/null || true
	@echo "Installing backend to $(INSTALL_BIN)..."
	@cp $(BIN_DIR)/$(APP_NAME) $(INSTALL_BIN)
	@chmod 755 $(INSTALL_BIN)
	@echo "Installing Flutter UI to $(INSTALL_DATA)..."
	@cp $(UI_DIR)/build/linux/x64/release/bundle/ui $(INSTALL_DATA)/ui.real
	@cp -r $(UI_DIR)/build/linux/x64/release/bundle/lib/. $(INSTALL_DATA)/lib/
	@cp -r $(UI_DIR)/build/linux/x64/release/bundle/data/. $(INSTALL_DATA)/data/
	@echo "Installing plugins to $(INSTALL_DATA)/plugins..."
	@for mod in cpu-temp gpu-temp network weather cpu-load gpu-load; do \
		if [ -f plugins/$$mod/$$mod ]; then \
			mkdir -p $(INSTALL_DATA)/plugins/$$mod; \
			cp plugins/$$mod/$$mod $(INSTALL_DATA)/plugins/$$mod/$$mod; \
		fi; \
	done
	@echo "Installing layout config to $(INSTALL_DATA)..."
	@mkdir -p $(INSTALL_DATA)/configs/layouts
	@cp configs/layouts/multi-page.yaml $(INSTALL_DATA)/configs/layouts/
	@echo "Restarting service..."
	@systemctl --user start "$(SYSTEMD_UNIT)"
	@echo "✓ Installed and restarted"

# Uninstall from user locations
uninstall:
	@systemctl --user stop "$(SYSTEMD_UNIT)" 2>/dev/null || true
	@rm -f $(INSTALL_BIN)
	@echo "✓ Uninstalled"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@rm -rf $(BIN_DIR) $(BUILD_DIR) $(DIST_DIR)
	@rm -f coverage.out coverage.html
	@rm -f $(APP_NAME) $(APP_NAME)-*
	@for p in cpu-temp gpu-temp network weather cpu-load gpu-load; do \
		rm -f plugins/$$p/$$p; \
	done
	@echo "✓ Cleaned"

# Clean Flutter UI build artifacts
clean-ui:
	@echo "Cleaning Flutter UI build artifacts..."
	@rm -rf $(UI_DIR)/build
	@echo "✓ Flutter UI cleaned"

# Additional development targets
.PHONY: fmt lint vet tidy generate-store

# Format code
fmt:
	@echo "Formatting code..."
	@go fmt ./...
	@echo "✓ Formatted"

# Run linter (requires golangci-lint)
lint:
	@if command -v golangci-lint > /dev/null; then \
		echo "Running linter..."; \
		golangci-lint run; \
	else \
		echo "Warning: golangci-lint not found. Install $(GOLANGCI_VERSION) from https://golangci-lint.run/"; \
	fi

# Run go vet
vet:
	@echo "Running go vet..."
	@go vet ./...
	@echo "✓ No issues found"

# Generate or preview CHANGELOG from git history
# Usage:
#   make changelog          — update CHANGELOG.md in place
#   make changelog TAG=v0.1.0  — preview notes for a specific (future) tag
changelog:
	@if ! command -v git-cliff > /dev/null; then \
		echo "git-cliff not found — install with: cargo install git-cliff"; exit 1; \
	fi
	@if [ -n "$(TAG)" ]; then \
		echo "Previewing release notes for $(TAG)..."; \
		git-cliff --tag "$(TAG)" --unreleased --strip all; \
	else \
		echo "Updating CHANGELOG.md..."; \
		git-cliff -o CHANGELOG.md; \
		echo "✓ CHANGELOG.md updated"; \
	fi

# Regenerate type-safe DB query code from schema.sql + queries/*.sql
generate-store:
	@if command -v sqlc > /dev/null || [ -f ~/go/bin/sqlc ]; then \
		SQLC=$$(command -v sqlc 2>/dev/null || echo ~/go/bin/sqlc); \
		INSTALLED=$$($$SQLC version 2>/dev/null); \
		if [ "$$INSTALLED" != "$(SQLC_VERSION)" ]; then \
			echo "Warning: sqlc $$INSTALLED installed, expected $(SQLC_VERSION)"; \
			echo "         Run: go install github.com/sqlc-dev/sqlc/cmd/sqlc@$(SQLC_VERSION)"; \
		fi; \
		echo "Regenerating store query code..."; \
		$$SQLC generate; \
		echo "✓ internal/store/db/ regenerated"; \
	else \
		echo "Error: sqlc not found. Install with: go install github.com/sqlc-dev/sqlc/cmd/sqlc@$(SQLC_VERSION)"; \
		exit 1; \
	fi

# Tidy dependencies
tidy:
	@echo "Tidying dependencies..."
	@go mod tidy
	@echo "✓ Dependencies tidied"
