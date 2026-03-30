from __future__ import annotations

import hmac
import json
import logging
import os
import platform
import time
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from dataclasses import asdict
from pathlib import Path
from typing import Any, Optional

from fastapi import Body, Depends, FastAPI, Header, HTTPException, Query, Request
from fastapi.concurrency import run_in_threadpool
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, HTMLResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, Field

from core.ark import Ark, sanitize_fts_query
from server.discovery import ArkServiceBroadcaster, ArkServiceDiscoverer, _get_local_ip
from server.pairing import claim_pairing, create_pairing, generate_device_id, parse_connection_string

logger = logging.getLogger(__name__)

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DB_PATH = Path(
    os.environ.get("LIFE_DB_PATH", str(PROJECT_ROOT / "life.db"))
).expanduser()
API_KEY = os.environ.get("LIFE_API_KEY")

db = Ark(DB_PATH, create=True)

# ---------------------------------------------------------------------------
# mDNS discovery (LAN sync)
# ---------------------------------------------------------------------------

MDNS_ENABLED = os.environ.get("ARK_MDNS", "1") != "0"
ARK_PORT = int(os.environ.get("ARK_PORT", "8000"))
ARK_DEVICE_NAME = os.environ.get("ARK_DEVICE_NAME", platform.node() or "ark")

_broadcaster: Optional[ArkServiceBroadcaster] = None
_discoverer: Optional[ArkServiceDiscoverer] = None


@asynccontextmanager
async def _lifespan(application: FastAPI) -> AsyncIterator[None]:
    global _broadcaster, _discoverer  # noqa: PLW0603
    if MDNS_ENABLED:
        try:
            _broadcaster = ArkServiceBroadcaster(port=ARK_PORT, device_name=ARK_DEVICE_NAME)
            _broadcaster.start()
            _discoverer = ArkServiceDiscoverer()
            _discoverer.start()
        except Exception:
            logger.warning("mDNS init failed, LAN discovery disabled", exc_info=True)

    # Start outbound peer connections (ARK_PEER_URLS)
    from server.peer_connector import peer_connector
    if os.environ.get("ARK_PEER_URLS", "").strip():
        peer_connector.start(db)

    yield

    peer_connector.stop()
    if _discoverer:
        _discoverer.stop()
    if _broadcaster:
        _broadcaster.stop()


app = FastAPI(title="Life DB API", version="0.1.0", lifespan=_lifespan)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=False,
    allow_methods=["*"],
    allow_headers=["*"],
)


def require_api_key(
    authorization: Optional[str] = Header(default=None),
    x_api_key: Optional[str] = Header(default=None, alias="X-API-Key"),
) -> None:
    if not API_KEY:
        # Fail closed: service should not be reachable without an auth secret.
        raise HTTPException(
            status_code=500, detail="Server is not configured (LIFE_API_KEY)."
        )

    token = None
    if x_api_key:
        token = x_api_key
    elif authorization and authorization.lower().startswith("bearer "):
        token = authorization.split(" ", 1)[1].strip()

    if not token or not hmac.compare_digest(token, API_KEY):
        raise HTTPException(status_code=401, detail="Unauthorized")


# =============================================================================
# Models
# =============================================================================


class EventIn(BaseModel):
    event_type: str
    data: dict[str, Any] = Field(default_factory=dict)
    category: Optional[str] = None
    occurred_at: Optional[str] = None
    summary: Optional[str] = None
    duration_seconds: Optional[int] = None
    timezone: Optional[str] = None
    source: Optional[str] = None
    source_id: Optional[str] = None
    device: Optional[str] = None
    tags: list[str] = Field(default_factory=list)
    entity_ids: list[str] = Field(default_factory=list)


class FilterIn(BaseModel):
    column: str
    operator: str
    value: Any


class PageRequestIn(BaseModel):
    offset: int = Field(default=0, ge=0, le=10000)
    limit: int = Field(default=100, ge=0, le=10000)
    sort_column: Optional[str] = None
    sort_direction: Optional[str] = None
    filters: Optional[list[FilterIn]] = None
    search: Optional[str] = None


class PageResponseOut(BaseModel):
    rows: list[dict[str, Any]]
    total_count: int
    has_more: bool
    query_time_ms: int


class CategoryStatsOut(BaseModel):
    category: str
    count: int
    event_types: dict[str, int]


class DayStatsOut(BaseModel):
    date: str
    count: int


class DbStatsOut(BaseModel):
    total_events: int
    total_entities: int
    categories: list[CategoryStatsOut]
    recent_days: list[DayStatsOut]


class CountRequestIn(BaseModel):
    filters: Optional[list[FilterIn]] = None
    search: Optional[str] = None


class TimeSeriesRequestIn(BaseModel):
    column: str = "occurred_at"
    bucket_size: str = "day"
    filters: Optional[list[FilterIn]] = None


class CategoricalRequestIn(BaseModel):
    column: str = "category"
    filters: Optional[list[FilterIn]] = None
    limit: Optional[int] = Field(default=None, ge=0, le=10000)


# =============================================================================
# Plugins
# =============================================================================


class PluginOut(BaseModel):
    id: str
    name: str
    description: str
    icon: str
    requires_api_key: bool
    api_key_configured: bool
    last_sync_at: Optional[str]
    last_sync_status: str
    last_sync_message: Optional[str]


class PluginSyncIn(BaseModel):
    # UI-friendly defaults: incremental sync for the last N days.
    days: Optional[int] = 30
    full_sync: bool = False


def _dict_row_factory(cursor: Any, row: Any) -> dict[str, Any]:
    return {cursor.description[i][0]: row[i] for i in range(len(cursor.description))}


def _get_last_import(source: str) -> Optional[dict[str, Any]]:
    with db.connection() as conn:
        saved_factory = conn.row_factory
        conn.row_factory = _dict_row_factory
        try:
            result = conn.execute(
                """
                SELECT
                  status,
                  started_at,
                  completed_at,
                  records_created,
                  records_updated,
                  records_skipped,
                  error_message
                FROM imports
                WHERE source = ?
                ORDER BY started_at DESC
                LIMIT 1
                """,
                (source,),
            ).fetchone()
        finally:
            conn.row_factory = saved_factory
        return result


def _import_to_status(
    row: Optional[dict[str, Any]],
) -> tuple[Optional[str], str, Optional[str]]:
    if not row:
        return None, "never", None

    status = str(row.get("status") or "")
    started_at = row.get("started_at")
    completed_at = row.get("completed_at")
    last_at = str(completed_at or started_at) if (completed_at or started_at) else None

    if status == "running":
        return last_at, "running", None
    if status == "completed":
        created = int(row.get("records_created") or 0)
        updated = int(row.get("records_updated") or 0)
        skipped = int(row.get("records_skipped") or 0)
        return (
            last_at,
            "success",
            f"created={created}, updated={updated}, skipped={skipped}",
        )
    if status == "failed":
        msg = row.get("error_message")
        return last_at, "error", str(msg) if msg else "failed"

    # pending/unknown
    return last_at, "never", None


# =============================================================================
# Helpers
# =============================================================================


def _build_where(
    filters: Optional[list[FilterIn]], search: Optional[str]
) -> tuple[str, list[Any]]:
    conditions = ["is_deleted = 0"]
    params: list[Any] = []

    valid_columns = {
        "id",
        "event_type",
        "category",
        "occurred_at",
        "created_at",
        "source",
        "summary",
    }

    if filters:
        for f in filters:
            if f.column not in valid_columns:
                continue

            col = f.column
            op = f.operator

            if op == "Equals":
                # UI uses a synthetic category label for NULLs.
                if col == "category" and str(f.value).lower() == "uncategorized":
                    conditions.append("(category IS NULL OR category = '')")
                else:
                    conditions.append(f"{col} = ?")
                    params.append("" if f.value is None else str(f.value))
            elif op == "NotEquals":
                if col == "category" and str(f.value).lower() == "uncategorized":
                    conditions.append("(category IS NOT NULL AND category != '')")
                else:
                    conditions.append(f"{col} != ?")
                    params.append("" if f.value is None else str(f.value))
            elif op == "Contains":
                conditions.append(f"{col} LIKE ?")
                params.append(f"%{'' if f.value is None else str(f.value)}%")
            elif op == "GreaterThan":
                conditions.append(f"{col} > ?")
                params.append("" if f.value is None else str(f.value))
            elif op == "LessThan":
                conditions.append(f"{col} < ?")
                params.append("" if f.value is None else str(f.value))
            elif op == "Between":
                if isinstance(f.value, list) and len(f.value) == 2:
                    conditions.append(f"{col} BETWEEN ? AND ?")
                    params.append("" if f.value[0] is None else str(f.value[0]))
                    params.append("" if f.value[1] is None else str(f.value[1]))
            elif op == "In":
                if isinstance(f.value, list) and len(f.value) > 0:
                    placeholders = ", ".join(["?"] * len(f.value))
                    conditions.append(f"{col} IN ({placeholders})")
                    params.extend(["" if v is None else str(v) for v in f.value])

    if search:
        s = sanitize_fts_query(search)
        if s:
            conditions.append(
                "rowid IN (SELECT rowid FROM events_fts WHERE events_fts MATCH ?)"
            )
            params.append(s)

    return " AND ".join(conditions), params


def _row_to_event_dict(row: Any) -> dict[str, Any]:
    tags_str = row.get("tags") or "[]"
    data_str = row.get("data") or "{}"
    try:
        tags = json.loads(tags_str)
    except Exception:
        tags = []
    try:
        data = json.loads(data_str)
    except Exception:
        data = {}

    return {
        "id": row.get("id"),
        "event_type": row.get("event_type"),
        "category": row.get("category"),
        "occurred_at": row.get("occurred_at"),
        "data": data,
        "summary": row.get("summary"),
        "source": row.get("source"),
        "tags": tags,
        "created_at": row.get("created_at"),
    }


# =============================================================================
# Basic
# =============================================================================


@app.get("/health")
def health() -> dict[str, Any]:
    return {"ok": True}


@app.get("/discovery", dependencies=[Depends(require_api_key)])
def get_discovery() -> dict[str, Any]:
    """Return Ark peers discovered on the LAN via mDNS."""
    peers = _discoverer.get_peers() if _discoverer else []
    return {
        "mdns_enabled": MDNS_ENABLED,
        "device_name": ARK_DEVICE_NAME,
        "peers": peers,
    }


@app.get("/stats/raw", dependencies=[Depends(require_api_key)])
def stats_raw() -> dict[str, Any]:
    # Raw core stats (useful for debugging).
    return db.get_stats()


# =============================================================================
# UI-friendly stats
# =============================================================================


@app.get("/db/stats", dependencies=[Depends(require_api_key)])
def get_db_stats() -> DbStatsOut:
    with db.connection() as conn:
        total_events = int(
            conn.execute("SELECT COUNT(*) FROM events WHERE is_deleted = 0").fetchone()[
                0
            ]
        )
        total_entities = int(
            conn.execute(
                "SELECT COUNT(*) FROM entities WHERE is_active = 1"
            ).fetchone()[0]
        )

        cat_map: dict[str, dict[str, Any]] = {}
        for cat, event_type, cnt in conn.execute(
            """
            SELECT COALESCE(category, 'uncategorized') as cat, event_type, COUNT(*) as cnt
            FROM events WHERE is_deleted = 0
            GROUP BY cat, event_type
            ORDER BY cat, cnt DESC
            """
        ):
            cat_s = str(cat)
            entry = cat_map.setdefault(
                cat_s, {"category": cat_s, "count": 0, "event_types": {}}
            )
            entry["event_types"][str(event_type)] = int(cnt)
            entry["count"] = int(entry["count"]) + int(cnt)

        categories = [
            CategoryStatsOut(**entry)
            for entry in sorted(cat_map.values(), key=lambda e: -int(e["count"]))
        ]

        recent_days: list[DayStatsOut] = []
        for date, cnt in conn.execute(
            """
            SELECT substr(occurred_at, 1, 10) as date, COUNT(*) as cnt
            FROM events
            WHERE is_deleted = 0 AND occurred_at >= date('now', '-30 days')
            GROUP BY date ORDER BY date DESC
            """
        ):
            recent_days.append(DayStatsOut(date=str(date), count=int(cnt)))

    return DbStatsOut(
        total_events=total_events,
        total_entities=total_entities,
        categories=categories,
        recent_days=recent_days,
    )


@app.get("/categories/{category}", dependencies=[Depends(require_api_key)])
def get_category_details(category: str) -> CategoryStatsOut:
    with db.connection() as conn:
        event_types: dict[str, int] = {}
        total = 0
        if category.lower() == "uncategorized":
            where = "category IS NULL OR category = ''"
            params = ()
        else:
            where = "category = ?"
            params = (category,)
        for event_type, cnt in conn.execute(
            f"""
            SELECT event_type, COUNT(*) as cnt
            FROM events
            WHERE is_deleted = 0 AND ({where})
            GROUP BY event_type
            ORDER BY cnt DESC
            """,  # noqa: S608
            params,
        ):
            total += int(cnt)
            event_types[str(event_type)] = int(cnt)

    return CategoryStatsOut(category=category, count=total, event_types=event_types)


# =============================================================================
# Events CRUD
# =============================================================================


@app.post("/events", dependencies=[Depends(require_api_key)])
def upsert_event(payload: EventIn) -> dict[str, Any]:
    try:
        event_id = db.record_event(
            payload.event_type,
            payload.data,
            category=payload.category,
            occurred_at=payload.occurred_at,
            summary=payload.summary,
            duration_seconds=payload.duration_seconds,
            timezone=payload.timezone,
            source=payload.source,
            source_id=payload.source_id,
            device=payload.device,
            tags=payload.tags,
            entity_ids=payload.entity_ids or None,
        )
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e

    return {"id": event_id}


@app.post("/events/batch", dependencies=[Depends(require_api_key)])
def upsert_events_batch(payload: list[EventIn]) -> dict[str, Any]:
    events = []
    for e in payload:
        events.append(
            {
                "event_type": e.event_type,
                "data": e.data,
                "category": e.category,
                "occurred_at": e.occurred_at,
                "summary": e.summary,
                "duration_seconds": e.duration_seconds,
                "timezone": e.timezone,
                "source": e.source,
                "source_id": e.source_id,
                "device": e.device,
                "tags": e.tags,
            }
        )

    try:
        created, updated, skipped = db.record_events_batch(events)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e

    return {"created": created, "updated": updated, "skipped": skipped}


@app.get("/events/{event_id}", dependencies=[Depends(require_api_key)])
def get_event(event_id: str) -> dict[str, Any]:
    event = db.get_event(event_id)
    if event is None:
        raise HTTPException(status_code=404, detail="Not found")
    return asdict(event)


@app.get("/events", dependencies=[Depends(require_api_key)])
def query_events(
    event_type: Optional[str] = None,
    category: Optional[str] = None,
    start_date: Optional[str] = None,
    end_date: Optional[str] = None,
    days: Optional[int] = None,
    source: Optional[str] = None,
    tags: Optional[str] = None,  # comma-separated
    search: Optional[str] = None,
    limit: int = Query(default=1000, ge=0, le=10000),
    offset: int = Query(default=0, ge=0, le=10000),
    order: str = "DESC",
) -> list[dict[str, Any]]:
    tags_list = None
    if tags:
        tags_list = [t.strip() for t in tags.split(",") if t.strip()]

    events = db.query_events(
        event_type=event_type,
        category=category,
        start_date=start_date,
        end_date=end_date,
        days=days,
        source=source,
        tags=tags_list,
        search=search,
        limit=limit,
        offset=offset,
        order=order,
    )
    return [asdict(e) for e in events]


@app.delete("/events/{event_id}", dependencies=[Depends(require_api_key)])
def delete_event(event_id: str) -> dict[str, Any]:
    deleted = db.delete_event(event_id)
    if not deleted:
        raise HTTPException(status_code=404, detail="Not found")
    return {"ok": True}


# =============================================================================
# UI queries (fast paging/analytics/search)
# =============================================================================


@app.post("/events/page", dependencies=[Depends(require_api_key)])
def get_page(request: PageRequestIn) -> PageResponseOut:
    start = time.time()

    where_clause, where_params = _build_where(request.filters, request.search)

    valid_sort_columns = {
        "id",
        "event_type",
        "category",
        "occurred_at",
        "created_at",
        "source",
        "summary",
    }
    sort_column = request.sort_column or "occurred_at"
    if sort_column not in valid_sort_columns:
        sort_column = "occurred_at"

    sort_direction = (request.sort_direction or "DESC").upper()
    if sort_direction not in {"ASC", "DESC"}:
        sort_direction = "DESC"

    with db.connection() as conn:
        saved_factory = conn.row_factory
        conn.row_factory = _dict_row_factory
        try:
            total_count = int(
                conn.execute(
                    f"SELECT COUNT(*) as cnt FROM events WHERE {where_clause}",  # noqa: S608
                    where_params,
                ).fetchone()["cnt"]
            )

            rows = conn.execute(
                f"""
                SELECT id, event_type, category, occurred_at, data, summary, source, tags, created_at
                FROM events
                WHERE {where_clause}
                ORDER BY {sort_column} {sort_direction}
                LIMIT ? OFFSET ?
                """,  # noqa: S608
                [*where_params, int(request.limit), int(request.offset)],
            ).fetchall()
        finally:
            conn.row_factory = saved_factory

    events = [_row_to_event_dict(r) for r in rows]
    has_more = request.offset + len(events) < total_count
    query_time_ms = int((time.time() - start) * 1000)
    return PageResponseOut(
        rows=events,
        total_count=total_count,
        has_more=has_more,
        query_time_ms=query_time_ms,
    )


@app.post("/events/count", dependencies=[Depends(require_api_key)])
def get_total_count(payload: CountRequestIn) -> int:
    where_clause, where_params = _build_where(payload.filters, payload.search)
    with db.connection() as conn:
        result = conn.execute(
            f"SELECT COUNT(*) FROM events WHERE {where_clause}",  # noqa: S608
            where_params,
        ).fetchone()
        return int(result[0]) if result else 0


@app.post("/analytics/time_series", dependencies=[Depends(require_api_key)])
def aggregate_time_series(payload: TimeSeriesRequestIn) -> list[dict[str, Any]]:
    where_clause, where_params = _build_where(payload.filters, None)

    date_format = {
        "hour": "%Y-%m-%d %H:00:00",
        "day": "%Y-%m-%d",
        "week": "%Y-%W",
        "month": "%Y-%m",
    }.get(payload.bucket_size, "%Y-%m-%d")

    with db.connection() as conn:
        rows = conn.execute(
            f"""
            SELECT strftime('{date_format}', occurred_at) as bucket, COUNT(*) as cnt
            FROM events WHERE {where_clause}
            GROUP BY bucket ORDER BY bucket
            """,  # noqa: S608
            where_params,
        ).fetchall()

    points: list[dict[str, Any]] = []
    for bucket, cnt in rows:
        points.append({"timestamp": bucket, "value": float(cnt), "count": int(cnt)})
    return points


@app.post("/analytics/categorical", dependencies=[Depends(require_api_key)])
def aggregate_categorical(payload: CategoricalRequestIn) -> list[dict[str, Any]]:
    where_clause, where_params = _build_where(payload.filters, None)

    valid_columns = {"category", "event_type", "source"}
    column = payload.column if payload.column in valid_columns else "category"
    limit_clause = f" LIMIT {int(payload.limit)}" if payload.limit else ""

    with db.connection() as conn:
        rows = conn.execute(
            f"""
            SELECT COALESCE({column}, 'unknown') as grp, COUNT(*) as cnt
            FROM events WHERE {where_clause}
            GROUP BY grp ORDER BY cnt DESC{limit_clause}
            """,  # noqa: S608
            where_params,
        ).fetchall()

    return [{"category": str(grp), "count": int(cnt)} for grp, cnt in rows]


@app.get("/search", dependencies=[Depends(require_api_key)])
def search_events(query: str, limit: int = Query(default=50, ge=0, le=10000)) -> list[dict[str, Any]]:
    q = sanitize_fts_query(query)
    if not q:
        return []

    with db.connection() as conn:
        saved_factory = conn.row_factory
        conn.row_factory = _dict_row_factory
        try:
            rows = conn.execute(
                """
                SELECT e.id, e.event_type, e.category, e.occurred_at, e.data, e.summary, e.source, e.tags, e.created_at
                FROM events e
                JOIN events_fts fts ON e.rowid = fts.rowid
                WHERE e.is_deleted = 0 AND events_fts MATCH ?
                ORDER BY rank
                LIMIT ?
                """,
                (q, int(limit)),
            ).fetchall()
        finally:
            conn.row_factory = saved_factory

    return [_row_to_event_dict(r) for r in rows]


# =============================================================================
# Plugins API
# =============================================================================


@app.get("/plugins", dependencies=[Depends(require_api_key)])
def list_plugins() -> list[PluginOut]:
    out: list[PluginOut] = []

    # Toggl Track
    toggl_token = os.environ.get("TOGGL_API_TOKEN") or os.environ.get("TOGGL_TRACK")
    last_row = _get_last_import("toggl_track")
    last_at, last_status, last_msg = _import_to_status(last_row)
    out.append(
        PluginOut(
            id="toggl_track",
            name="Toggl Track",
            description="Импорт записей учёта времени из Toggl Track",
            icon="clock",
            requires_api_key=True,
            api_key_configured=bool(toggl_token),
            last_sync_at=last_at,
            last_sync_status=last_status,
            last_sync_message=last_msg,
        )
    )

    return out


@app.post("/plugins/{plugin_id}/sync", dependencies=[Depends(require_api_key)])
async def sync_plugin(
    plugin_id: str,
    payload: PluginSyncIn = Body(default=PluginSyncIn()),
) -> dict[str, Any]:
    if plugin_id != "toggl_track":
        raise HTTPException(status_code=404, detail="Unknown plugin")

    # Import lazily: avoids importing plugin code unless needed.
    from plugins.toggl_track import TogglAPIError, TogglQuotaError, TogglTrackPlugin

    token = os.environ.get("TOGGL_API_TOKEN") or os.environ.get("TOGGL_TRACK")
    if not token:
        raise HTTPException(
            status_code=400, detail="TOGGL_API_TOKEN is not configured on the server"
        )

    def _run_sync() -> dict[str, Any]:
        plugin = TogglTrackPlugin(token)
        created, updated, skipped = plugin.sync_to_ark(
            db,
            days=payload.days,
            full_sync=payload.full_sync,
        )
        projects = plugin.create_project_entities(db)
        clients = plugin.create_client_entities(db)
        return {
            "created": int(created),
            "updated": int(updated),
            "skipped": int(skipped),
            "projects_created": int(projects),
            "clients_created": int(clients),
        }

    try:
        return await run_in_threadpool(_run_sync)
    except TogglQuotaError as e:
        raise HTTPException(
            status_code=429, detail=f"Rate limit: wait {e.wait_seconds}s"
        ) from e
    except TogglAPIError as e:
        raise HTTPException(status_code=502, detail=str(e)) from e


# =============================================================================
# Device Pairing
# =============================================================================


class PairingClaimIn(BaseModel):
    code: str
    device_name: str = "Unknown Device"
    platform: str = "unknown"


@app.post("/pairing/create", dependencies=[Depends(require_api_key)])
def pairing_create() -> dict[str, Any]:
    """Generate a new pairing code + QR for device onboarding."""
    if not API_KEY:
        raise HTTPException(status_code=500, detail="LIFE_API_KEY not configured")
    lan_ip = _get_local_ip()
    server_url = f"http://{lan_ip}:{ARK_PORT}"
    result = create_pairing(server_url, API_KEY, ARK_DEVICE_NAME)
    return result


@app.post("/pairing/claim")
def pairing_claim(body: PairingClaimIn) -> dict[str, Any]:
    """
    Claim a pairing code. No auth required — the code IS the credential.

    Registers the device in sync_devices and returns connection info.
    """
    payload = claim_pairing(body.code)
    if payload is None:
        raise HTTPException(status_code=404, detail="Invalid or expired pairing code")

    device_id = generate_device_id()

    # Register in sync_devices table
    from server.sync_ws import _ensure_sync_tables, _register_device

    _ensure_sync_tables(db)
    _register_device(db, device_id, body.device_name, body.platform)

    return {
        "server_url": payload["server_url"],
        "api_key": payload["api_key"],
        "device_id": device_id,
    }


@app.get("/pairing/qr")
def pairing_qr(request: Request) -> HTMLResponse:
    """
    Return an HTML page with QR code and connection string for instant pairing.

    Access restricted to localhost — the connection string contains the API key.
    """
    # Restrict to localhost: this endpoint exposes the API key without auth.
    _LOCALHOST_HOSTS = {"127.0.0.1", "::1", "localhost", "testclient"}
    client_host = request.client.host if request.client else None
    if client_host is not None and client_host not in _LOCALHOST_HOSTS:
        raise HTTPException(status_code=403, detail="QR pairing page is only accessible from localhost")
    if not API_KEY:
        raise HTTPException(status_code=500, detail="LIFE_API_KEY not configured")

    lan_ip = _get_local_ip()
    server_url = f"http://{lan_ip}:{ARK_PORT}"
    result = create_pairing(server_url, API_KEY, ARK_DEVICE_NAME)
    conn = result["connection_string"]

    html = f"""\
<!DOCTYPE html>
<html lang="ru">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Ark — Подключение</title>
  <style>
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      display: flex; justify-content: center; align-items: center;
      min-height: 100vh; margin: 0;
      background: #0a0a0a; color: #e0e0e0;
    }}
    .card {{
      text-align: center; padding: 2rem;
      background: #1a1a1a; border-radius: 16px;
      box-shadow: 0 4px 24px rgba(0,0,0,0.4);
      max-width: 420px;
    }}
    h1 {{ font-size: 1.4rem; margin-bottom: 0.25rem; }}
    .conn {{
      font-size: 0.95rem; font-family: monospace;
      color: #60a5fa; margin: 1rem 0; padding: 0.75rem;
      background: #111; border-radius: 8px;
      word-break: break-all; cursor: pointer;
      border: 1px solid #333; transition: border-color 0.2s;
    }}
    .conn:hover {{ border-color: #60a5fa; }}
    .copied {{ color: #34d399; font-size: 0.8rem; margin-top: 0.25rem; }}
    img {{ max-width: 280px; border-radius: 8px; margin-top: 1rem; }}
    .hint {{ color: #888; font-size: 0.85rem; margin-top: 1rem; }}
  </style>
</head>
<body>
  <div class="card">
    <h1>Ark</h1>
    <p>Отсканируй QR или скопируй код подключения</p>
    <div class="conn" onclick="navigator.clipboard.writeText(this.textContent.trim()).then(()=>{{document.getElementById('cp').style.display='block';setTimeout(()=>document.getElementById('cp').style.display='none',2000)}})">{conn}</div>
    <div id="cp" class="copied" style="display:none">Скопировано!</div>
    <img src="{result["qr_data_url"]}" alt="QR code">
    <p class="hint">Вставь код в Delphi / Elysium — и готово</p>
  </div>
</body>
</html>"""
    return HTMLResponse(content=html)


# =============================================================================
# WebSocket sync
# =============================================================================

from server.sync_ws import init_sync, router as sync_router

init_sync(db)
app.include_router(sync_router)

from server.peer_server import init_peer_sync, router as peer_router

init_peer_sync(db)
app.include_router(peer_router)

# =============================================================================
# Optional: serve the built web UI (ui/dist) from the same process.
# =============================================================================


UI_DIST = PROJECT_ROOT / "ui" / "dist"
if UI_DIST.exists():
    assets_dir = UI_DIST / "assets"
    if assets_dir.exists():
        app.mount("/assets", StaticFiles(directory=assets_dir), name="assets")

    @app.get("/", include_in_schema=False)
    def _ui_index() -> FileResponse:
        return FileResponse(UI_DIST / "index.html")

    @app.get("/{path:path}", include_in_schema=False)
    def _ui_spa(path: str) -> FileResponse:
        # Serve real files if they exist, otherwise fall back to SPA entrypoint.
        file_path = (UI_DIST / path).resolve()
        if not file_path.is_relative_to(UI_DIST.resolve()):
            return FileResponse(UI_DIST / "index.html")
        if file_path.is_file():
            return FileResponse(file_path)
        return FileResponse(UI_DIST / "index.html")
