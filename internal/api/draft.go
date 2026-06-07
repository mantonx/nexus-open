package api

import (
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/mantonx/nexus-next/internal/zone"
)

const draftIdleTimeout = 60 * time.Second

// DraftManager holds an in-memory draft layout that Flutter can mutate and
// preview live before committing to the DB. A single draft is active at a
// time; it is auto-discarded after draftIdleTimeout of inactivity or when
// all WS clients disconnect.
type DraftManager struct {
	mu       sync.Mutex
	draft    *zone.Config
	reloader LayoutReloader
	broadcast func(WSMessage)
	timer    *time.Timer
}

// NewDraftManager creates a DraftManager wired to the given live-preview reloader
// and WS broadcast function.
func NewDraftManager(reloader LayoutReloader, broadcast func(WSMessage)) *DraftManager {
	return &DraftManager{reloader: reloader, broadcast: broadcast}
}

// HasDraft reports whether a draft is currently open.
func (dm *DraftManager) HasDraft() bool {
	dm.mu.Lock()
	defer dm.mu.Unlock()
	return dm.draft != nil
}

// GetDraft returns a deep copy of the current draft, or nil if no draft is open.
func (dm *DraftManager) GetDraft() *zone.Config {
	dm.mu.Lock()
	defer dm.mu.Unlock()
	if dm.draft == nil {
		return nil
	}
	return deepCopyConfig(dm.draft)
}

// OpenDraft initialises a draft from the committed config. If a draft is already
// open it is returned as-is (no data is lost). Returns a copy of the draft.
func (dm *DraftManager) OpenDraft(committed *zone.Config) *zone.Config {
	dm.mu.Lock()
	defer dm.mu.Unlock()
	if dm.draft == nil {
		dm.draft = deepCopyConfig(committed)
		dm.resetTimerLocked()
	}
	return deepCopyConfig(dm.draft)
}

// UpdateDraft replaces the draft with cfg, previews it live, broadcasts
// draft_state, and resets the idle timer.
func (dm *DraftManager) UpdateDraft(cfg *zone.Config) error {
	dm.mu.Lock()
	dm.draft = deepCopyConfig(cfg)
	snap := deepCopyConfig(dm.draft)
	dm.resetTimerLocked()
	dm.mu.Unlock()

	if err := dm.reloader.ReloadFromConfig(snap); err != nil {
		return fmt.Errorf("live preview failed: %w", err)
	}
	dm.broadcastDraftState(snap)
	return nil
}

// Commit writes the current draft to the DB via commitFn and clears the draft.
// commitFn should persist the config and return any error.
func (dm *DraftManager) Commit(commitFn func(*zone.Config) error) error {
	dm.mu.Lock()
	if dm.draft == nil {
		dm.mu.Unlock()
		return fmt.Errorf("no draft to commit")
	}
	snap := deepCopyConfig(dm.draft)
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

// Discard abandons the draft and reverts the live display to the committed config.
func (dm *DraftManager) Discard(revertTo *zone.Config) {
	dm.mu.Lock()
	if dm.draft == nil {
		dm.mu.Unlock()
		return
	}
	dm.draft = nil
	dm.stopTimerLocked()
	dm.mu.Unlock()

	if revertTo != nil {
		_ = dm.reloader.ReloadFromConfig(revertTo)
	}
	dm.broadcast(WSMessage{Type: "draft_state", Data: map[string]any{"active": false}})
}

// resetTimerLocked restarts the idle timer. Must be called with dm.mu held.
func (dm *DraftManager) resetTimerLocked() {
	if dm.timer != nil {
		// Stop + drain to avoid the timer firing while we're resetting.
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

// deepCopyConfig returns a full deep copy via JSON round-trip.
func deepCopyConfig(c *zone.Config) *zone.Config {
	if c == nil {
		return nil
	}
	b, err := json.Marshal(c)
	if err != nil {
		panic("deepCopyConfig: marshal: " + err.Error())
	}
	var out zone.Config
	if err := json.Unmarshal(b, &out); err != nil {
		panic("deepCopyConfig: unmarshal: " + err.Error())
	}
	return &out
}
