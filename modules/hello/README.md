# Hello Module

A simple example module that demonstrates the Nexus Open module interface.

## Building

```bash
cd modules/hello
go build -o hello .
```

## Testing

```bash
# Test standalone (won't work - needs plugin host)
./hello

# Test via host
nexus-open module test exec:./modules/hello/hello
```

## What it does

- Displays "Hello!" as the primary text
- Shows "Example Module" as secondary text
- Includes a sample sparkline
- Updates every 2 seconds

## Module Interface

This module implements the `module.Module` interface:

```go
type Module interface {
    Describe() (Descriptor, error)
    Sample() (Payload, error)
}
```

It communicates with the host via RPC using HashiCorp go-plugin.
