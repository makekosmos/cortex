# Server API (VPS)

Goal: one database on your VPS, and any app (todo app, Toggl sync, scripts) can read/write fast via HTTP.

## What you get

- HTTP API for events (create/update/query/delete)
- One shared secret key (no “many keys”)
- Swagger UI: `/docs` (OpenAPI spec at `/openapi.json`)
- Optional built-in web UI (served from `ui/dist` at `/` after `bun run build` in `ui/`)

## Plugins (optional)

The server can run some import plugins and track them in the `imports` table.

Currently supported:
- `toggl_track` (env: `TOGGL_API_TOKEN`)

Endpoints (authorized with the same `LIFE_API_KEY`):
- `GET /plugins` (status + last sync)
- `POST /plugins/toggl_track/sync` (manual sync; default last 30 days)

## Configure

Environment variables:
- `LIFE_DB_PATH` - path to the SQLite file on the VPS
- `LIFE_API_KEY` - shared secret key (keep it private)

## Run (dev)

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r server/requirements.txt

export LIFE_DB_PATH=/var/lib/life/life.db
export LIFE_API_KEY='change-me-to-a-long-random-secret'

uvicorn server.app:app --host 0.0.0.0 --port 8000
```

Open:
- `http://<your-vps>:8000/docs`

## Quick examples

Create or update an event (idempotent when `source+source_id` are stable):

```bash
curl -X POST 'http://<your-vps>:8000/events' \
  -H 'Authorization: Bearer YOUR_KEY' \
  -H 'Content-Type: application/json' \
  -d '{
    "event_type": "todo_task",
    "category": "todo",
    "source": "todo-app",
    "source_id": "task-123",
    "data": {"title": "Buy milk", "status": "open"},
    "tags": ["home"]
  }'
```

Query:

```bash
curl 'http://<your-vps>:8000/events?category=todo&limit=50' \
  -H 'Authorization: Bearer YOUR_KEY'
```

Batch (fast sync after offline):

```bash
curl -X POST 'http://<your-vps>:8000/events/batch' \
  -H 'Authorization: Bearer YOUR_KEY' \
  -H 'Content-Type: application/json' \
  -d '[{"event_type":"todo_task","source":"todo-app","source_id":"task-1","data":{"status":"open"}}]'
```
