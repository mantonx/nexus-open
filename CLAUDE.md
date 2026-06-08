# Nexus Open — Claude Instructions

## Before writing any code

**State the root cause in one sentence. If you can't, investigate first.**

Do not write a fix until you understand why the problem exists. "Try this" is not debugging.

## Before adding any new UI or rendering path

Ask: does the existing system already handle this? The Flutter companion UI receives the hardware framebuffer as a live frame stream over WebSocket. Any visual change rendered to the 640×48 hardware display is automatically mirrored in Flutter. Do not build parallel rendering paths.

## Pace

Do not let user frustration accelerate you into reactive mode. Frustration is a signal to slow down and think, not to produce faster. One correct action after understanding beats three fast wrong ones.

## Build and deploy

Use `make install` for a full production deploy (Go + Flutter + plugins + service restart).

For iterative development, use `make dev` instead — it starts the full hot-reload environment automatically via overmind. The SessionStart hook also starts it when a Claude session opens.

- **Go changes**: air rebuilds the daemon and all plugins on save (~2–4 s) automatically.
- **Flutter changes**: watchexec detects `.dart` saves and sends `SIGUSR1` to flutter run — hot reload happens in under a second with no manual input. Only new imports require a hot-restart (`kill -USR2 $(cat /tmp/nexus-flutter.pid)`).
- **Attach to a process**: `overmind connect backend`, `overmind connect ui`, or `overmind connect ui-reload` (detach with `Ctrl-B D`).
- **Stop dev servers**: `overmind quit` when done for the day.

Only fall back to `make install` when changing the layout YAML, adding a new plugin for the first time, or making changes that affect the installed binary path.

## Lock ordering

When acquiring multiple mutexes in the zone package, always acquire in this order:

1. `configMu`
2. `lastFrameMu`
3. `detailMu`
4. `payloadsMu`

Never hold a broader lock while acquiring a narrower one in reverse order.
