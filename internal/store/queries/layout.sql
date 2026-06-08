-- name: ListPages :many
SELECT id, name, ord FROM pages ORDER BY ord;

-- name: InsertPage :execlastid
INSERT INTO pages(name, ord) VALUES(?, ?);

-- name: UpdatePage :exec
UPDATE pages SET name = ?, ord = ? WHERE id = ?;

-- name: DeletePage :exec
DELETE FROM pages WHERE id = ?;

-- name: UpdatePageOrd :exec
UPDATE pages SET ord = ? WHERE id = ?;

-- name: CountPages :one
SELECT COUNT(*) FROM pages;

-- name: DeleteAllPages :exec
DELETE FROM pages;

-- name: InsertPageWithID :exec
INSERT INTO pages(id, name, ord) VALUES(?, ?, ?);

-- name: ListZonesForPage :many
SELECT id, page_id, ord, width_px, plugin, refresh_ms, align,
       on_tap, choices_json, config_json, theme_json
FROM zones
WHERE page_id = ?
ORDER BY ord;

-- name: GetZonePageID :one
SELECT page_id FROM zones WHERE id = ?;

-- name: GetZoneConfigJSON :one
SELECT config_json FROM zones WHERE id = ?;

-- name: UpdateZoneConfigJSON :exec
UPDATE zones SET config_json = ? WHERE id = ?;

-- name: InsertZone :exec
INSERT INTO zones(id, page_id, ord, width_px, plugin, refresh_ms, align,
                  on_tap, choices_json, config_json, theme_json)
VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);

-- name: UpdateZone :execresult
UPDATE zones
SET page_id = ?, ord = ?, width_px = ?, plugin = ?, refresh_ms = ?, align = ?,
    on_tap = ?, choices_json = ?, config_json = ?, theme_json = ?
WHERE id = ?;

-- name: DeleteZone :exec
DELETE FROM zones WHERE id = ?;

-- name: UpdateZoneOrd :exec
UPDATE zones SET ord = ? WHERE id = ? AND page_id = ?;

-- name: UpdateZonePlugin :exec
UPDATE zones SET plugin = ? WHERE id = ?;
