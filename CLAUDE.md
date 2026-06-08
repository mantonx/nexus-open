# Nexus Open — Claude Instructions

## Before writing any code

**State the root cause in one sentence. If you can't, investigate first.**

Do not write a fix until you understand why the problem exists. "Try this" is not debugging.

## Before adding any new UI or rendering path

Ask: does the existing system already handle this? The Flutter companion UI receives the hardware framebuffer as a live frame stream over WebSocket. Any visual change rendered to the 640×48 hardware display is automatically mirrored in Flutter. Do not build parallel rendering paths.

## Pace

Do not let user frustration accelerate you into reactive mode. Frustration is a signal to slow down and think, not to produce faster. One correct action after understanding beats three fast wrong ones.

## Build and deploy

Always use `make install` — never `flutter build`, `go build`, or manual binary copies in isolation. The install target builds Go + Flutter + plugins + copies everything + restarts the service atomically.

## Lock ordering

When acquiring multiple mutexes in the zone package, always acquire in this order:
1. `configMu`
2. `lastFrameMu`
3. `detailMu`
4. `payloadsMu`

Never hold a broader lock while acquiring a narrower one in reverse order.
