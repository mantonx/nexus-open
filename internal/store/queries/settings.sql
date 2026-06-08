-- name: GetSetting :one
SELECT value FROM settings WHERE key = ?;

-- name: GetAllSettings :many
SELECT key, value FROM settings ORDER BY key;

-- name: UpsertSetting :exec
INSERT INTO settings(key, value) VALUES(?, ?)
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
