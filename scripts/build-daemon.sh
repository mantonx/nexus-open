#!/usr/bin/env bash
set -e

VERSION=$(git describe --tags --match 'v*' --always 2>/dev/null | sed 's/^v//;s/-[0-9]*-g[0-9a-f]*//' || echo "0.0.0-dev")
COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_TIME=$(date -u '+%Y-%m-%d_%H:%M:%S')

exec go build -trimpath \
  -ldflags "-X main.version=${VERSION} -X main.commit=${COMMIT} -X main.buildTime=${BUILD_TIME}" \
  -o ./tmp/nexus-open \
  ./cmd/nexus-open
