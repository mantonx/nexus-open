# Nexus Open - Makefile
# Standardized build system for all targets

.PHONY: help build build-debug build-release build-ui build-plugins build-all test test-race coverage clean clean-ui install uninstall run run-tray dev deb appimage rpm generate-api models all

# Configuration
APP_NAME := nexus-open
VERSION := 1.0.0
COMMIT := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_TIME := $(shell date -u '+%Y-%m-%d_%H:%M:%S')

# Directories
BIN_DIR := bin
BUILD_DIR := build
DIST_DIR := dist
CMD_DIR := cmd/nexus-open
UI_DIR := ui
UI_BUILD_DIR := $(UI_DIR)/build/linux/x64/release/bundle

# Build flags
LDFLAGS := -X main.version=$(VERSION) -X main.commit=$(COMMIT) -X main.buildTime=$(BUILD_TIME)
GOFLAGS := -trimpath
GO_BUILD := CGO_ENABLED=1 go build

# Default target
help:
	@echo "Nexus Open - Build System"
	@echo ""
	@echo "UI Testing:"
	@echo "  make screenshot-tour - Navigate all tabs and capture screenshots"
	@echo ""
	@echo "Development:"
	@echo "  make build         - Build Go backend only (with debug info)"
	@echo "  make build-ui      - Build Flutter UI only"
	@echo "  make build-plugins - Build all external plugins"
	@echo "  make build-all     - Build backend, UI, and all plugins"
	@echo "  make build-release - Build optimized release binary (stripped)"
	@echo "  make run           - Build and run Go backend only"
	@echo "  make run-tray      - Build and run bundled app with system tray"
	@echo "  make dev           - Run with live reload (requires air)"
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
	@echo "  make models        - Generate freezed/json_serializable Dart models"
	@echo "  make all           - Build all packages"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean         - Remove build artifacts"
	@echo "  make install       - Install to /usr/local/bin (requires sudo)"
	@echo "  make uninstall     - Remove from /usr/local/bin (requires sudo)"
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
	@cd $(UI_DIR) && flutter build linux --release
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

# Development mode with live reload (requires github.com/air-verse/air)
dev:
	@if command -v air > /dev/null || [ -f ~/go/bin/air ]; then \
		if command -v air > /dev/null; then \
			air; \
		else \
			~/go/bin/air; \
		fi; \
	else \
		echo "Error: 'air' not found. Install with: go install github.com/air-verse/air@latest"; \
		exit 1; \
	fi

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
		echo "Error: go-openapi not found. Install with: go install github.com/go-openapi/cmd/go-openapi@latest"; \
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

# Install to system
install: build-release
	@echo "Installing $(APP_NAME) to /usr/local/bin..."
	@sudo cp $(BIN_DIR)/$(APP_NAME) /usr/local/bin/
	@sudo chmod 755 /usr/local/bin/$(APP_NAME)
	@echo "✓ Installed to /usr/local/bin/$(APP_NAME)"

# Uninstall from system
uninstall:
	@echo "Removing $(APP_NAME) from /usr/local/bin..."
	@sudo rm -f /usr/local/bin/$(APP_NAME)
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
.PHONY: fmt lint vet tidy

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
		echo "Warning: golangci-lint not found. Install from https://golangci-lint.run/"; \
	fi

# Run go vet
vet:
	@echo "Running go vet..."
	@go vet ./...
	@echo "✓ No issues found"

# Tidy dependencies
tidy:
	@echo "Tidying dependencies..."
	@go mod tidy
	@echo "✓ Dependencies tidied"
