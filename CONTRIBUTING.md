# Contributing to Nexus Open

Thanks for taking an interest in contributing! This is a hardware interfacing project — contributions that improve device compatibility, add new modules, or improve the Linux desktop experience are especially welcome.

## Getting Started

1. **Fork** the repo and clone your fork
2. **Read** [DEVELOPMENT.md](DEVELOPMENT.md) for the local development setup
3. **Open an issue** before starting significant work — it avoids duplicate effort and lets us align on direction

## Development Setup

```bash
# Prerequisites: Go 1.24+, Flutter 3.24+, libusb-1.0-dev
git clone https://github.com/mantonx/nexus-next.git
cd nexus-next

# Run without hardware (mock device mode)
NEXUS_MOCK_DEVICE=1 make run

# Run tests
make test

# Run with race detector
make test-race
```

See [DEVICE_SETUP.md](DEVICE_SETUP.md) if you have a physical Corsair iCUE Nexus to test against.

## Making Changes

- **Branch** off `main` with a descriptive name (`feat/brightness-control`, `fix/touch-swipe-regression`)
- **Keep PRs focused** — one feature or fix per PR
- **Tests** — add tests for new behaviour; maintain 60%+ overall coverage (`make coverage`)
- **No comments on obvious code** — only comment on non-obvious constraints, workarounds, or invariants

## Writing a Module

Modules are standalone binaries that communicate with the host over gRPC (via [hashicorp/go-plugin](https://github.com/hashicorp/go-plugin)). See `modules/hello/main.go` for the minimal example and `pkg/module/types.go` for the `Payload` type.

```
modules/
  your-module/
    main.go        # Implements pkg/module.Module interface
```

Build your module binary and reference it from a layout YAML under `configs/layouts/`.

## Submitting a Pull Request

1. Ensure `make test` and `make vet` pass
2. Describe **what** changed and **why** in the PR description
3. Reference any related issue with `Fixes #123` or `Relates to #123`
4. Hardware testing notes are appreciated but not required — the mock device covers most paths

## Code Style

- Standard Go formatting (`gofmt` / `make fmt`)
- `log/slog` for logging — no `fmt.Println` in library code
- Interfaces over concrete types at package boundaries
- Context-based cancellation for anything that blocks

## Questions

Open a [GitHub Discussion](https://github.com/mantonx/nexus-next/discussions) for design questions or anything that doesn't fit an issue.
