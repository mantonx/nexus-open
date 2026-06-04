# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| latest  | ✅        |

This project is in active development. Security fixes are applied to the latest version only.

## Scope

Nexus Open runs as a **local desktop application** with the following attack surface:

- **HTTP API on localhost:1985** — no authentication by default; intended for local use only. Do not expose this port to a network.
- **USB HID communication** with the Corsair iCUE Nexus device
- **Plugin binaries** loaded from `~/.config/nexus-open/plugins/` — only load plugins from sources you trust

## Reporting a Vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Report security issues by emailing: **matthew.panton@gmail.com**

Include:
- A description of the vulnerability and its potential impact
- Steps to reproduce
- Any suggested fix (optional but appreciated)

You can expect an acknowledgement within 48 hours and a resolution timeline within 7 days for confirmed vulnerabilities.

## Notes

- The API server binds to `127.0.0.1` by default. If you change this, you are responsible for adding appropriate authentication.
- Module plugins are executed as child processes with the same privileges as the main process. Vet third-party plugins carefully.
