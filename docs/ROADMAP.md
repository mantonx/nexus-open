# Nexus Open — Roadmap

## Current Version: 1.0.0

**Status:** v1.0 feature-complete. All core functionality is working and the
improvement plan has been executed. See [IMPROVEMENT_PLAN.md](../IMPROVEMENT_PLAN.md)
for a full task-by-task record of what was done.

---

## What's in v1.0

- Display rendering at 30 FPS with four live modules (CPU temp, GPU temp,
  network, weather)
- Flutter settings UI with NavigationRail, dark mode, and live 640×48 preview
  via WebSocket
- Per-module configuration (graph type, units) through the Modules tab
- Actionable USB error messages (permission denied, device busy, not found)
- REST API with OpenAPI 3.0 spec at `/openapi.yaml`
- System tray integration with Flutter readiness check
- DEB, AppImage, RPM packaging; CI with linting, Flutter analysis, spec drift
  check, and tag-triggered release workflow
- `NEXUS_MOCK_DEVICE=1` for development without hardware

---

## v1.1 — Polish and Accessibility

Items deferred from the improvement plan that are well-scoped for the next
minor release:

### From the improvement plan

- **5.3/5.4** — Add `freezed` + `json_serializable` typed models from the spec;
  migrate `NexusApiService` serialization to use them. (`openapi-generator` was
  evaluated and rejected: all Dart-native wrappers require Java or force Dio/Chopper.
  `freezed` is the right fit for a small, stable, locally-controlled API.)
- **6.2** — First-run onboarding overlay (Welcome → Connect → Location → Done)
- **6.3** — Module error visibility: surface per-zone error state through the
  API and in the Modules tab card
- **7.2** — Flutter widget tests for `SettingsPage`, `PreviewTab`, `ModulesTab`
- **4C.3** — Full 8px spacing grid and visual design system pass
- **9.1** — App icon set (SVG source + 16/48/64/128/256px exports)
- **9.2** — Screenshots for README and Flatpak metainfo
- **11.1–11.4** — Accessibility: `Semantics` wrappers, keyboard navigation,
  WCAG AA contrast audit, system text scaling

### Packaging

- **3.2** — AUR submission
- **3.4** — Complete Flatpak metadata (Flutter UI module, icon, screenshots)

---

## v1.2 — Advanced Features

- **Layout editor** — drag-to-resize zones in the Flutter UI
- **Slideshow mode** — rotate through background images on a timer
- **Auto-brightness** — time-of-day brightness schedule
- **Auto-start toggle** — UI control for `systemctl --user enable nexus-open`
- **Additional modules** — media player (now playing), disk usage, CPU load graph

---

## Post-v1.0 Considerations

- **Multi-device support** — architectural change; needs design work
- **NixOS / Gentoo packaging** — community contribution welcome
- **Plugin sandboxing / checksum verification** — security hardening
- **WebSocket touch event mirroring** — stream touch events to Flutter UI
- **Windows / macOS ports** — requires replacing HID backend

---

## Contributing

Pick any open item, open a GitHub issue to claim it, and submit a PR. See
[CONTRIBUTING.md](../CONTRIBUTING.md) for development setup.

---

**Last updated:** 2026-06-04
