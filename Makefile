# Nexus Open - Makefile
# Standardized build system for all targets

.PHONY: help build build-debug build-release test test-race coverage clean install uninstall run dev deb appimage all

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

# Build flags
LDFLAGS := -X main.version=$(VERSION) -X main.commit=$(COMMIT) -X main.buildTime=$(BUILD_TIME)
GOFLAGS := -trimpath
GO_BUILD := CGO_ENABLED=1 go build

# Default target
help:
	@echo "Nexus Open - Build System"
	@echo ""
	@echo "Development:"
	@echo "  make build         - Build development binary (with debug info)"
	@echo "  make build-debug   - Build with debug symbols (same as build)"
	@echo "  make build-release - Build optimized release binary (stripped)"
	@echo "  make run           - Build and run the application"
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

# Run the application
run: build
	@echo "Running $(APP_NAME)..."
	@$(BIN_DIR)/$(APP_NAME)

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

# Build all packages
all: deb appimage
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
	@echo "✓ Cleaned"

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
