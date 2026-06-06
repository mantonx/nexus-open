// Package store provides a SQLite-backed configuration store for Nexus Open.
//
// All persistent configuration lives in a single database file:
//
//	~/.config/nexus-open/nexus.db
//
// Three logical areas are stored here:
//   - settings table: flat key/value pairs for UI preferences (colours,
//     locale, display config). Replaces config.yaml + shared_preferences.
//   - zone_plugin_config table: per-zone plugin configuration overrides
//     (graph types, units, etc.). Replaces zone-configs.yaml.
//   - pages + zones tables: the hardware display layout (which modules
//     appear where). Replaces configs/layouts/*.yaml for user-edited config;
//     the YAML layout files remain as the factory default / import source.
//
// On first run the store imports existing YAML files so users don't lose
// their configuration when upgrading.
package store

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"

	_ "modernc.org/sqlite"
)

const currentSchemaVersion = 4

// DB is the application's SQLite store. All methods are safe for concurrent use.
type DB struct {
	db     *sql.DB
	mu     sync.RWMutex
	logger *slog.Logger
	path   string
}

// Open opens (or creates) the SQLite database at path and runs any pending
// schema migrations. If path is empty it defaults to
// ~/.config/nexus-open/nexus.db.
func Open(path string, logger *slog.Logger) (*DB, error) {
	if logger == nil {
		logger = slog.Default()
	}

	if path == "" {
		configDir, err := os.UserConfigDir()
		if err != nil {
			return nil, fmt.Errorf("store: get config dir: %w", err)
		}
		path = filepath.Join(configDir, "nexus-open", "nexus.db")
	}

	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return nil, fmt.Errorf("store: create dir: %w", err)
	}

	db, err := sql.Open("sqlite", path+"?_journal=WAL&_timeout=5000")
	if err != nil {
		return nil, fmt.Errorf("store: open: %w", err)
	}

	// Single writer to avoid SQLITE_BUSY under concurrent writes.
	db.SetMaxOpenConns(1)

	// modernc.org/sqlite ignores the _fk DSN param; enforce foreign keys explicitly.
	if _, err := db.Exec(`PRAGMA foreign_keys = ON`); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("store: enable foreign keys: %w", err)
	}

	s := &DB{db: db, logger: logger, path: path}

	if err := s.migrate(); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("store: migrate: %w", err)
	}

	logger.Info("store opened", "path", path)
	return s, nil
}

// Close closes the underlying database connection.
func (s *DB) Close() error {
	return s.db.Close()
}

// Path returns the filesystem path of the database file.
func (s *DB) Path() string {
	return s.path
}

// IsFirstRun reports whether the database was freshly created on this open
// (i.e. schema_version didn't exist before migrate ran).
func (s *DB) IsFirstRun() bool {
	var v int
	row := s.db.QueryRow(`SELECT version FROM schema_version`)
	if err := row.Scan(&v); err != nil || v == 0 {
		return true
	}
	return false
}

// ── Settings (key/value) ──────────────────────────────────────────────────────

// GetSetting returns the stored value for key, or defaultVal if not found.
func (s *DB) GetSetting(key, defaultVal string) string {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var val string
	err := s.db.QueryRow(
		`SELECT value FROM settings WHERE key = ?`, key,
	).Scan(&val)
	if err != nil {
		return defaultVal
	}
	return val
}

// SetSetting upserts a setting key/value pair.
func (s *DB) SetSetting(key, value string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(
		`INSERT INTO settings(key, value) VALUES(?, ?)
		 ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
		key, value,
	)
	return err
}

// GetSettings returns all settings as a map.
func (s *DB) GetSettings() (map[string]string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query(`SELECT key, value FROM settings`)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()

	m := make(map[string]string)
	for rows.Next() {
		var k, v string
		if err := rows.Scan(&k, &v); err != nil {
			return nil, err
		}
		m[k] = v
	}
	return m, rows.Err()
}

// SetSettings upserts multiple settings in a single transaction.
func (s *DB) SetSettings(settings map[string]string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback() //nolint:errcheck

	stmt, err := tx.Prepare(
		`INSERT INTO settings(key, value) VALUES(?, ?)
		 ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
	)
	if err != nil {
		return err
	}
	defer func() { _ = stmt.Close() }()

	for k, v := range settings {
		if _, err := stmt.Exec(k, v); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// ── Zone plugin config ────────────────────────────────────────────────────────

// GetZoneConfig returns the stored plugin config for zoneID, or nil.
func (s *DB) GetZoneConfig(zoneID string) (map[string]interface{}, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var raw string
	err := s.db.QueryRow(
		`SELECT config_json FROM zone_plugin_config WHERE zone_id = ?`, zoneID,
	).Scan(&raw)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}

	var m map[string]interface{}
	if err := json.Unmarshal([]byte(raw), &m); err != nil {
		return nil, err
	}
	return m, nil
}

// SetZoneConfig upserts the plugin config for zoneID.
func (s *DB) SetZoneConfig(zoneID string, cfg map[string]interface{}) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	raw, err := json.Marshal(cfg)
	if err != nil {
		return err
	}

	_, err = s.db.Exec(
		`INSERT INTO zone_plugin_config(zone_id, config_json) VALUES(?, ?)
		 ON CONFLICT(zone_id) DO UPDATE SET config_json = excluded.config_json`,
		zoneID, string(raw),
	)
	return err
}

// DeleteZoneConfig removes the stored config for zoneID.
func (s *DB) DeleteZoneConfig(zoneID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	_, err := s.db.Exec(`DELETE FROM zone_plugin_config WHERE zone_id = ?`, zoneID)
	return err
}

// GetAllZoneConfigs returns all stored zone configs.
func (s *DB) GetAllZoneConfigs() (map[string]map[string]interface{}, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.db.Query(`SELECT zone_id, config_json FROM zone_plugin_config`)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()

	result := make(map[string]map[string]interface{})
	for rows.Next() {
		var zoneID, raw string
		if err := rows.Scan(&zoneID, &raw); err != nil {
			return nil, err
		}
		var m map[string]interface{}
		if err := json.Unmarshal([]byte(raw), &m); err != nil {
			s.logger.Warn("store: malformed zone config", "zone_id", zoneID, "error", err)
			continue
		}
		result[zoneID] = m
	}
	return result, rows.Err()
}

// ── Schema migrations ─────────────────────────────────────────────────────────

func (s *DB) migrate() error {
	// Bootstrap: create schema_version if this is a brand new file.
	if _, err := s.db.Exec(`
		CREATE TABLE IF NOT EXISTS schema_version (
			version INTEGER NOT NULL
		);
		INSERT INTO schema_version(version)
		SELECT 0 WHERE NOT EXISTS (SELECT 1 FROM schema_version);
	`); err != nil {
		return fmt.Errorf("bootstrap schema_version: %w", err)
	}

	var current int
	if err := s.db.QueryRow(`SELECT version FROM schema_version`).Scan(&current); err != nil {
		return fmt.Errorf("read schema version: %w", err)
	}

	migrations := []func(*sql.Tx) error{
		migration1, // v0 → v1: settings + zone_plugin_config
		migration2, // v1 → v2: pages + zones (layout editor)
		migration3, // v2 → v3: rename zone_module_config → zone_plugin_config
		migration4, // v3 → v4: rewrite exec:./modules/ → exec:./plugins/ after directory rename
	}

	for i := current; i < currentSchemaVersion; i++ {
		s.logger.Info("store: running migration", "version", i+1)
		tx, err := s.db.Begin()
		if err != nil {
			return err
		}
		if err := migrations[i](tx); err != nil {
			tx.Rollback() //nolint:errcheck
			return fmt.Errorf("migration %d: %w", i+1, err)
		}
		if _, err := tx.Exec(`UPDATE schema_version SET version = ?`, i+1); err != nil {
			tx.Rollback() //nolint:errcheck
			return err
		}
		if err := tx.Commit(); err != nil {
			return err
		}
		s.logger.Info("store: migration complete", "version", i+1)
	}

	return nil
}

// migration1 creates the initial schema (v0 → v1).
func migration1(tx *sql.Tx) error {
	_, err := tx.Exec(`
		-- UI preferences (replaces config.yaml + shared_preferences)
		CREATE TABLE IF NOT EXISTS settings (
			key   TEXT PRIMARY KEY,
			value TEXT NOT NULL
		);

		-- Per-zone module configuration (replaces zone-configs.yaml)
		CREATE TABLE IF NOT EXISTS zone_plugin_config (
			zone_id     TEXT PRIMARY KEY,
			config_json TEXT NOT NULL DEFAULT '{}'
		);
	`)
	return err
}

// migration2 adds the layout tables: pages and zones (v1 → v2).
func migration2(tx *sql.Tx) error {
	_, err := tx.Exec(`
		-- Display pages (swipeable screens)
		CREATE TABLE IF NOT EXISTS pages (
			id   INTEGER PRIMARY KEY AUTOINCREMENT,
			name TEXT    NOT NULL,
			ord  INTEGER NOT NULL DEFAULT 0
		);

		-- Zones within a page (left-to-right, must sum to 640px)
		CREATE TABLE IF NOT EXISTS zones (
			id           TEXT    PRIMARY KEY,
			page_id      INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
			ord          INTEGER NOT NULL DEFAULT 0,
			width_px     INTEGER NOT NULL CHECK(width_px >= 80 AND width_px <= 640),
			module       TEXT    NOT NULL DEFAULT 'builtin:placeholder',
			refresh_ms   INTEGER NOT NULL DEFAULT 2000,
			align        TEXT    NOT NULL DEFAULT 'center',
			config_json  TEXT    NOT NULL DEFAULT '{}',
			theme_json   TEXT    NOT NULL DEFAULT '{}'
		);

		CREATE INDEX IF NOT EXISTS zones_page_ord ON zones(page_id, ord);
	`)
	return err
}

// migration3 renames zone_module_config → zone_plugin_config (v2 → v3).
// Databases created after migration1 was updated already have zone_plugin_config,
// so this is a no-op for them; only legacy DBs with zone_module_config need renaming.
func migration3(tx *sql.Tx) error {
	var n int
	if err := tx.QueryRow(
		`SELECT count(*) FROM sqlite_master WHERE type='table' AND name='zone_module_config'`,
	).Scan(&n); err != nil {
		return err
	}
	if n == 0 {
		return nil
	}
	_, err := tx.Exec(`ALTER TABLE zone_module_config RENAME TO zone_plugin_config`)
	return err
}

// migration4 rewrites exec:./modules/ plugin paths to exec:./plugins/ (v3 → v4).
// The modules/ directory was renamed to plugins/ in the codebase; any layout
// seeded before that rename will have stale paths that fail to launch.
func migration4(tx *sql.Tx) error {
	_, err := tx.Exec(`
		UPDATE zones
		SET module = 'exec:./plugins/' || substr(module, length('exec:./modules/') + 1)
		WHERE module LIKE 'exec:./modules/%'
	`)
	return err
}
