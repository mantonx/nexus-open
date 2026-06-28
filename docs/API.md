# REST API

The daemon listens on `127.0.0.1:1985`. Full OpenAPI spec at `/openapi.yaml` or `api/openapi.yaml`.

| Endpoint                     | Description                                 |
| ---------------------------- | ------------------------------------------- |
| `GET /api/health`            | Health check and device status              |
| `GET /api/config`            | Settings                                    |
| `POST /api/config`           | Update settings                             |
| `GET /api/layout`            | Current layout (pages + zones)              |
| `GET /api/plugins`           | Plugin catalog with per-zone status         |
| `GET /api/zones/{id}/status` | Zone health and last error                  |
| `GET /api/device/info`       | Connection state                            |
| `GET /api/ws`                | WebSocket — live 640×48 RGBA frames         |
