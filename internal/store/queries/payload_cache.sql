-- name: UpsertPayloadCache :exec
INSERT INTO payload_cache(zone_id, plugin_id, payload, fetched_at)
VALUES(?, ?, ?, ?)
ON CONFLICT(zone_id) DO UPDATE SET
    plugin_id  = excluded.plugin_id,
    payload    = excluded.payload,
    fetched_at = excluded.fetched_at;

-- name: GetPayloadCache :one
SELECT plugin_id, payload, fetched_at FROM payload_cache WHERE zone_id = ?;

-- name: DeletePayloadCache :exec
DELETE FROM payload_cache WHERE zone_id = ?;
