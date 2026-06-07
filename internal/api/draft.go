package api

import (
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/mantonx/nexus-next/internal/zone"
)

// draftIdleTimeout is how long an open draft persists without any activity
// before it is silently discarded. Long enough that normal editing sessions
// (including walking away for coffee) don't lose work.
const draftIdleTimeout = 30 * time.Minute

// DraftManager holds an in-memory draft layout that Flutter can mutate and
// preview live before committing to the DB. A single draft is active at a
// time; it is auto-discarded after draftIdleTimeout of inactivity or when
// all WS clients disconnect.
type DraftManager struct {
	mu        sync.Mutex
	draft     *zone.Config
	store     LayoutStore    // authoritative source — read on every OpenDraft
	reloader  LayoutReloader // live preview only — never queried for config
	broadcast func(WSMessage)
	timer     *time.Timer
}

// NewDraftManager creates a DraftManager wired to the given store, live-preview
// reloader, and WS broadcast function.
func NewDraftManager(store LayoutStore, reloader LayoutReloader, broadcast func(WSMessage)) *DraftManager {
	return &DraftManager{store: store, reloader: reloader, broadcast: broadcast}
}

// HasDraft reports whether a draft is currently open.
func (dm *DraftManager) HasDraft() bool {
	dm.mu.Lock()
	defer dm.mu.Unlock()
	return dm.draft != nil
}

// GetDraft returns a copy of the current draft, or nil if no draft is open.
func (dm *DraftManager) GetDraft() *zone.Config {
	dm.mu.Lock()
	defer dm.mu.Unlock()
	if dm.draft == nil {
		return nil
	}
	return zone.CopyConfig(dm.draft)
}

// OpenDraft returns the active draft. If none is open it reads the full layout
// from the DB (including empty pages) and initialises a fresh draft from it.
// Normalizes plugin IDs on the way out so the Flutter UI always sees clean IDs.
// Returns a copy — mutating the returned config does not affect the stored draft.
func (dm *DraftManager) OpenDraft() (*zone.Config, error) {
	dm.mu.Lock()
	defer dm.mu.Unlock()

	if dm.draft == nil {
		committed, err := dm.loadFromStore()
		if err != nil {
			return nil, fmt.Errorf("draft: open from store: %w", err)
		}
		dm.draft = committed
		dm.resetTimerLocked()
	}

	out := zone.CopyConfig(dm.draft)
	normalizeConfigPluginIDs(out)
	return out, nil
}

// UpdateDraft replaces the stored draft with cfg, previews a render-safe
// version live (empty pages stripped), broadcasts draft_state, and resets
// the idle timer.
func (dm *DraftManager) UpdateDraft(cfg *zone.Config) error {
	dm.mu.Lock()
	dm.draft = zone.CopyConfig(cfg)
	dm.resetTimerLocked()
	dm.mu.Unlock()

	// Build a render-safe snapshot: zone.Manager rejects pages with no zones.
	renderSnap := renderSafeConfig(cfg)
	if renderSnap != nil {
		if err := dm.reloader.ReloadFromConfig(renderSnap); err != nil {
			return fmt.Errorf("live preview failed: %w", err)
		}
	}

	dm.broadcastDraftState(cfg)
	return nil
}

// Commit writes the current draft to the DB via commitFn and clears the draft.
func (dm *DraftManager) Commit(commitFn func(*zone.Config) error) error {
	dm.mu.Lock()
	if dm.draft == nil {
		dm.mu.Unlock()
		return fmt.Errorf("no draft to commit")
	}
	snap := zone.CopyConfig(dm.draft)
	dm.mu.Unlock()

	if err := commitFn(snap); err != nil {
		return err
	}

	dm.mu.Lock()
	dm.draft = nil
	dm.stopTimerLocked()
	dm.mu.Unlock()

	dm.broadcast(WSMessage{Type: "draft_state", Data: map[string]any{"active": false}})
	return nil
}

// Discard abandons the draft and reverts the live display to the committed
// state read fresh from the DB.
func (dm *DraftManager) Discard() {
	dm.mu.Lock()
	if dm.draft == nil {
		dm.mu.Unlock()
		return
	}
	dm.draft = nil
	dm.stopTimerLocked()
	dm.mu.Unlock()

	// Revert live display to whatever is currently in the DB.
	if committed, err := dm.loadFromStore(); err == nil {
		renderSnap := renderSafeConfig(committed)
		if renderSnap != nil {
			_ = dm.reloader.ReloadFromConfig(renderSnap)
		}
	}
	dm.broadcast(WSMessage{Type: "draft_state", Data: map[string]any{"active": false}})
}

// ── helpers ───────────────────────────────────────────────────────────────────

// loadFromStore reads the full layout from the DB, including empty pages.
// Must NOT be called with dm.mu held (store calls may block).
func (dm *DraftManager) loadFromStore() (*zone.Config, error) {
	pages, zoneMap, err := dm.store.GetFullLayout()
	if err != nil {
		return nil, err
	}

	cfg := &zone.Config{
		Name:    "User Layout",
		Version: "1.0",
		Theme:   zone.DefaultTheme(),
		Nav:     zone.NavConfig{SwipeEnabled: true},
	}

	// Preserve theme/nav from the live renderer if available — the DB doesn't
	// store global theme (only per-zone overrides).
	if live := dm.reloader.GetConfig(); live != nil {
		cfg.Theme = live.Theme
		cfg.Nav = live.Nav
	}

	for _, p := range pages {
		page := zone.Page{Name: p.Name}
		for _, z := range zoneMap[p.ID] {
			zc := zone.ZoneConfig{
				ID:           z.ID,
				Width:        z.WidthPx,
				Plugin:       zone.NormalizePluginID(z.Plugin),
				RefreshMs:    z.RefreshMs,
				Align:        zone.Alignment(z.Align),
				PluginConfig: z.ConfigJSON,
			}
			if len(z.ThemeJSON) > 0 {
				raw, err := json.Marshal(z.ThemeJSON)
				if err == nil {
					var t zone.Theme
					if err := json.Unmarshal(raw, &t); err == nil {
						if t.Accent != "" || t.Bg != "" || t.Fg != "" || t.Muted != "" {
							zc.ThemeOverride = &t
						}
					}
				}
			}
			page.Zones = append(page.Zones, zc)
		}
		cfg.Pages = append(cfg.Pages, page)
	}

	if len(cfg.Pages) == 0 {
		return nil, fmt.Errorf("draft: store has no pages")
	}
	return cfg, nil
}

// renderSafeConfig returns a copy of cfg with empty pages removed, suitable
// for passing to ReloadFromConfig. Returns nil if no non-empty pages remain.
func renderSafeConfig(cfg *zone.Config) *zone.Config {
	out := zone.CopyConfig(cfg)
	kept := out.Pages[:0]
	for _, p := range out.Pages {
		if len(p.Zones) > 0 {
			kept = append(kept, p)
		}
	}
	out.Pages = kept
	if len(out.Pages) == 0 {
		return nil
	}
	return out
}

// resetTimerLocked restarts the idle timer. Must be called with dm.mu held.
func (dm *DraftManager) resetTimerLocked() {
	if dm.timer != nil {
		if !dm.timer.Stop() {
			select {
			case <-dm.timer.C:
			default:
			}
		}
		dm.timer.Reset(draftIdleTimeout)
	} else {
		dm.timer = time.AfterFunc(draftIdleTimeout, dm.onIdleTimeout)
	}
}

// stopTimerLocked cancels the idle timer. Must be called with dm.mu held.
func (dm *DraftManager) stopTimerLocked() {
	if dm.timer != nil {
		if !dm.timer.Stop() {
			select {
			case <-dm.timer.C:
			default:
			}
		}
		dm.timer = nil
	}
}

func (dm *DraftManager) onIdleTimeout() {
	dm.mu.Lock()
	if dm.draft == nil {
		dm.mu.Unlock()
		return
	}
	dm.draft = nil
	dm.timer = nil
	dm.mu.Unlock()
	dm.broadcast(WSMessage{Type: "draft_state", Data: map[string]any{"active": false, "reason": "idle_timeout"}})
}

func (dm *DraftManager) broadcastDraftState(cfg *zone.Config) {
	if cfg == nil {
		dm.broadcast(WSMessage{Type: "draft_state", Data: map[string]any{"active": false}})
		return
	}
	dm.broadcast(WSMessage{Type: "draft_state", Data: map[string]any{
		"active": true,
		"layout": cfg,
	}})
}
