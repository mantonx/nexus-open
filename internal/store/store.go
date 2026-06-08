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
	"context"
	"database/sql"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"

	dbgen "github.com/mantonx/nexus-open/internal/store/db"
	_ "modernc.org/sqlite"
)

const currentSchemaVersion = 7

// DB is the application's SQLite store. All methods are safe for concurrent use.
type DB struct {
	db       *sql.DB
	q        *dbgen.Queries
	mu       sync.RWMutex
	logger   *slog.Logger
	path     string
	firstRun bool // true if schema_version was 0 when Open() was called (before migrate ran)
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

	// Capture pre-migration schema version so IsFirstRun() can report correctly
	// after migrate() has already bumped the version.
	var preVer int
	_ = db.QueryRow(`SELECT version FROM schema_version LIMIT 1`).Scan(&preVer)

	s := &DB{db: db, q: dbgen.New(db), logger: logger, path: path, firstRun: preVer == 0}

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

// IsFirstRun reports whether the database was freshly created on this open.
// The answer is captured before migrate() runs so it remains correct even
// after migrations have bumped the schema version to the current value.
func (s *DB) IsFirstRun() bool {
	return s.firstRun
}


// ── Settings (key/value) ──────────────────────────────────────────────────────

// GetSetting returns the stored value for key, or defaultVal if not found.
func (s *DB) GetSetting(key, defaultVal string) string {
	s.mu.RLock()
	defer s.mu.RUnlock()

	val, err := s.q.GetSetting(context.Background(), key)
	if err != nil {
		return defaultVal
	}
	return val
}

// SetSetting upserts a setting key/value pair.
func (s *DB) SetSetting(key, value string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.q.UpsertSetting(context.Background(), dbgen.UpsertSettingParams{
		Key:   key,
		Value: value,
	})
}

// GetSettings returns all settings as a map.
func (s *DB) GetSettings() (map[string]string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	rows, err := s.q.GetAllSettings(context.Background())
	if err != nil {
		return nil, err
	}
	m := make(map[string]string, len(rows))
	for _, r := range rows {
		m[r.Key] = r.Value
	}
	return m, nil
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

	qtx := s.q.WithTx(tx)
	for k, v := range settings {
		if err := qtx.UpsertSetting(context.Background(), dbgen.UpsertSettingParams{
			Key: k, Value: v,
		}); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// ── Schema migrations ───────────────────────────��─────────────────────────────

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
		migrateV1_settingsAndZonePluginConfig,      // v0 → v1
		migrateV2_pagesAndZones,                    // v1 → v2
		migrateV3_renameTable,                      // v2 → v3
		migrateV4_rewriteExecPaths,                 // v3 → v4
		migrateV5_consolidateConfigAndRenameColumn, // v4 → v5
		migrateV6_addOnTapAndChoicesJson,           // v5 → v6
		migrateV7_payloadCache,                     // v6 → v7
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

// migrateV1_settingsAndZonePluginConfig creates the initial schema (v0 → v1).
func migrateV1_settingsAndZonePluginConfig(tx *sql.Tx) error {
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

// migrateV2_pagesAndZones adds the layout tables: pages and zones (v1 → v2).
func migrateV2_pagesAndZones(tx *sql.Tx) error {
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

// migrateV3_renameTable renames zone_module_config → zone_plugin_config (v2 → v3).
// Databases created after migrateV1 was updated already have zone_plugin_config,
// so this is a no-op for them; only legacy DBs with zone_module_config need renaming.
func migrateV3_renameTable(tx *sql.Tx) error {
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

// migrateV4_rewriteExecPaths rewrites exec:./modules/ plugin paths to exec:./plugins/ (v3 → v4).
// The modules/ directory was renamed to plugins/ in the codebase; any layout
// seeded before that rename will have stale paths that fail to launch.
func migrateV4_rewriteExecPaths(tx *sql.Tx) error {
	_, err := tx.Exec(`
		UPDATE zones
		SET module = 'exec:./plugins/' || substr(module, length('exec:./modules/') + 1)
		WHERE module LIKE 'exec:./modules/%'
	`)
	return err
}

// migrateV5_consolidateConfigAndRenameColumn consolidates per-zone plugin config
// into a single location and renames the module column to plugin (v4 → v5).
//
// Before this migration two tables both held per-zone plugin config:
//   - zones.config_json   (written by the layout API)
//   - zone_plugin_config  (written by the zone config manager and sampler)
//
// They could diverge silently. This migration merges them: any row in
// zone_plugin_config that has a non-empty config is merged into
// zones.config_json (zones wins on conflict since it is the newer store),
// then zone_plugin_config is dropped.
//
// It also renames the zones.module column to zones.plugin to match the Go
// struct field name (StoredZone.Plugin) and the plugin: terminology used
// everywhere since the v1.0 rename.
func migrateV5_consolidateConfigAndRenameColumn(tx *sql.Tx) error {
	// Merge zone_plugin_config rows into zones.config_json where the zone
	// exists but zones.config_json is still the empty default '{}'. Rows
	// where zones.config_json already has content are left untouched (the
	// layout API value is considered authoritative).
	if _, err := tx.Exec(`
		UPDATE zones
		SET config_json = (
			SELECT zpc.config_json
			FROM zone_plugin_config zpc
			WHERE zpc.zone_id = zones.id
			  AND zpc.config_json != '{}'
			  AND zpc.config_json != 'null'
		)
		WHERE id IN (
			SELECT zpc.zone_id
			FROM zone_plugin_config zpc
			WHERE zpc.config_json != '{}'
			  AND zpc.config_json != 'null'
		)
		AND (config_json = '{}' OR config_json = 'null' OR config_json = '')
	`); err != nil {
		return fmt.Errorf("migration5: merge zone_plugin_config: %w", err)
	}

	// Drop the now-redundant table. zone_plugin_config entries that used the
	// "plugin:" key prefix (plugin-level defaults stored by SetPluginDefault)
	// are not migrated — that feature is superseded by the Phase 1 config
	// schema declared in Descriptor.ConfigSchema.
	if _, err := tx.Exec(`DROP TABLE IF EXISTS zone_plugin_config`); err != nil {
		return fmt.Errorf("migration5: drop zone_plugin_config: %w", err)
	}

	// Rename zones.module → zones.plugin. SQLite supports RENAME COLUMN
	// since 3.25.0 (2018-09-15); modernc.org/sqlite bundles a recent version.
	if _, err := tx.Exec(`ALTER TABLE zones RENAME COLUMN module TO plugin`); err != nil {
		return fmt.Errorf("migration5: rename module column: %w", err)
	}

	return nil
}

// migrateV6_addOnTapAndChoicesJson adds on_tap and choices_json columns to zones (v5 → v6).
func migrateV6_addOnTapAndChoicesJson(tx *sql.Tx) error {
	if _, err := tx.Exec(`ALTER TABLE zones ADD COLUMN on_tap TEXT NOT NULL DEFAULT ''`); err != nil {
		return fmt.Errorf("migration6: add on_tap: %w", err)
	}
	if _, err := tx.Exec(`ALTER TABLE zones ADD COLUMN choices_json TEXT NOT NULL DEFAULT '[]'`); err != nil {
		return fmt.Errorf("migration6: add choices_json: %w", err)
	}
	return nil
}

// migrateV7_payloadCache adds the payload_cache table for persisting OnTap
// DetailPayload results across restarts (v6 → v7).
func migrateV7_payloadCache(tx *sql.Tx) error {
	_, err := tx.Exec(`
		CREATE TABLE IF NOT EXISTS payload_cache (
			zone_id    TEXT    PRIMARY KEY,
			plugin_id  TEXT    NOT NULL,
			payload    TEXT    NOT NULL,
			fetched_at INTEGER NOT NULL
		);
	`)
	return err
}
