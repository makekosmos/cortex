"""
WebSocket sync endpoint for Ark.

Protocol (see SYNC.md):
  1. Client connects to /ws/sync?key=<api_key>
  2. Client sends sync_start with device_id, device_name, platform, vector
  3. Server responds with changes the client hasn't seen
  4. Client sends its unseen changes
  5. Realtime: client sends changes, server broadcasts to others
"""

from __future__ import annotations

import asyncio
import hmac
import json
import logging
import os
import uuid
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, WebSocket, WebSocketDisconnect

from core.ark import Ark

logger = logging.getLogger(__name__)

router = APIRouter()


# ---------------------------------------------------------------------------
# Connection manager
# ---------------------------------------------------------------------------

class ConnectionManager:
    """Tracks active WebSocket connections by device_id."""

    def __init__(self) -> None:
        self._connections: dict[str, WebSocket] = {}
        self._device_info: dict[str, dict[str, str]] = {}

    @property
    def connections(self) -> dict[str, WebSocket]:
        return self._connections

    async def connect(
        self, device_id: str, websocket: WebSocket, device_name: str = "", platform: str = ""
    ) -> None:
        # If same device reconnects, close old connection silently
        if device_id in self._connections:
            try:
                await self._connections[device_id].close(code=4001, reason="replaced")
            except Exception:
                logger.debug("Connection cleanup error", exc_info=True)
        self._connections[device_id] = websocket
        self._device_info[device_id] = {"name": device_name, "platform": platform}

    def disconnect(self, device_id: str) -> None:
        self._connections.pop(device_id, None)
        self._device_info.pop(device_id, None)

    async def broadcast(self, message: dict[str, Any], *, exclude: Optional[str] = None) -> None:
        """Send message to all connected clients except `exclude`."""
        dead: list[str] = []
        for did, ws in self._connections.items():
            if did == exclude:
                continue
            try:
                await ws.send_json(message)
            except Exception:
                dead.append(did)
        for did in dead:
            self.disconnect(did)


manager = ConnectionManager()


# ---------------------------------------------------------------------------
# Sync helpers (direct SQL — core/sync.py doesn't exist yet)
# ---------------------------------------------------------------------------

def _ensure_sync_tables(db: Ark) -> None:
    """Create sync tables if they don't exist."""
    import uuid as _uuid
    with db.connection() as conn:
        conn.executescript("""
            CREATE TABLE IF NOT EXISTS sync_devices (
                device_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                platform TEXT NOT NULL,
                last_seen_at TEXT,
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );

            CREATE TABLE IF NOT EXISTS sync_vectors (
                device_id TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                last_seq INTEGER NOT NULL,
                PRIMARY KEY (device_id, peer_id)
            );

            CREATE TABLE IF NOT EXISTS sync_outbox (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL,
                change_type TEXT NOT NULL,
                data TEXT NOT NULL,
                device_id TEXT NOT NULL,
                device_seq INTEGER NOT NULL,
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                synced INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS sync_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        """)
        # Generate a stable server epoch on first init — changes only when DB is wiped/recreated.
        # Clients compare this value; on mismatch they know the server was reset and push all local data.
        existing = conn.execute("SELECT value FROM sync_meta WHERE key='server_epoch'").fetchone()
        if not existing:
            epoch = str(_uuid.uuid4())
            conn.execute("INSERT INTO sync_meta(key, value) VALUES('server_epoch', ?)", (epoch,))
            conn.commit()


def _register_device(db: Ark, device_id: str, name: str, platform: str) -> None:
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
    with db.connection() as conn:
        conn.execute(
            """
            INSERT INTO sync_devices (device_id, name, platform, last_seen_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(device_id) DO UPDATE SET
                name = excluded.name,
                platform = excluded.platform,
                last_seen_at = excluded.last_seen_at
            """,
            (device_id, name, platform, now),
        )


def _get_server_vector(db: Ark) -> dict[str, int]:
    """Return the server's view: max device_seq per device_id from outbox."""
    with db.connection() as conn:
        rows = conn.execute(
            "SELECT device_id, MAX(device_seq) as max_seq FROM sync_outbox GROUP BY device_id"
        ).fetchall()
    return {str(row[0]): int(row[1]) for row in rows}


def _full_sync_from_events(db: Ark) -> list[dict[str, Any]]:
    """Build sync_changes from the events table (source of truth).

    Called when client sends sync_start with empty vector — they need a full
    snapshot. Returns the authoritative current state of all tasks/projects/areas/tags.
    Each change uses the latest outbox metadata (device_id/seq) for that source_id
    so clients can track version vectors correctly.
    """
    import os as _os, platform as _platform

    server_id = _os.environ.get("ARK_DEVICE_ID", f"ark-server-{_platform.node()}")

    # Build map: lowercase source_id -> (device_id, device_seq) from the latest outbox entry
    with db.connection() as conn:
        outbox_rows = conn.execute(
            "SELECT id, device_id, device_seq, json_extract(data,'$.source_id') as src_id "
            "FROM sync_outbox WHERE json_extract(data,'$.source_id') IS NOT NULL "
            "AND json_extract(data,'$.source_id') != '' "
            "ORDER BY id"  # ascending so last entry wins
        ).fetchall()

    outbox_meta: dict[str, tuple[str, int, int]] = {}
    for row in outbox_rows:
        src_id = (row[3] or "").lower()
        if src_id:
            outbox_meta[src_id] = (str(row[1]), int(row[2]), int(row[0]))

    # Fetch all current events (rowid for fallback seq)
    with db.connection() as conn:
        events = conn.execute(
            "SELECT rowid, event_type, source, source_id, summary, occurred_at, data "
            "FROM events WHERE is_deleted=0 "
            "ORDER BY rowid"
        ).fetchall()

    changes: list[dict[str, Any]] = []
    seen_source_ids: set[str] = set()

    for ev in events:
        ev_source_id = (ev[3] or "").lower()
        event_type = str(ev[1])

        # Skip duplicates (same source_id already processed — events table can have
        # entries from multiple sources for the same logical object)
        if ev_source_id and ev_source_id in seen_source_ids:
            continue
        if ev_source_id:
            seen_source_ids.add(ev_source_id)

        # Parse inner task data
        raw_data = ev[6]
        try:
            data_dict = json.loads(raw_data) if isinstance(raw_data, str) else (raw_data or {})
        except (json.JSONDecodeError, TypeError):
            data_dict = {}

        # Normalize id field to lowercase inside task data
        if isinstance(data_dict, dict):
            for field in ("id", "source_id", "projectId", "headingId", "areaId"):
                if data_dict.get(field):
                    data_dict[field] = str(data_dict[field]).lower()

        change_data: dict[str, Any] = {
            "event_type": event_type,
            "category": "productivity",
            "source": ev[2] or "",
            "source_id": ev_source_id,
            "summary": ev[4] or "",
            "occurred_at": ev[5] or "",
            "data": data_dict,
        }

        # Use outbox metadata for device_id/seq (for vector tracking)
        if ev_source_id in outbox_meta:
            device_id, device_seq, _ = outbox_meta[ev_source_id]
        else:
            device_id = server_id
            device_seq = int(ev[0])

        changes.append({
            "event_id": ev_source_id or str(ev[0]),
            "change_type": "create",
            "data": change_data,
            "device_id": device_id,
            "device_seq": device_seq,
            "created_at": ev[5] or "",
        })

    return changes


def _get_changes_since(db: Ark, client_vector: dict[str, int]) -> list[dict[str, Any]]:
    """Return outbox entries the client hasn't seen based on its vector.

    Two modes:
    - Empty vector: full snapshot from events table (authoritative, normalized UUIDs)
    - Non-empty vector: incremental delta from outbox with LWW deduplication
    """
    if not client_vector:
        return _full_sync_from_events(db)

    # Incremental sync: deduplicate outbox by (event_type, source_id lower)
    # keeping the entry with the latest occurred_at. Entries with no occurred_at
    # lose to entries that have one ONLY if both have different isCompleted state;
    # to handle mac entries (no occurred_at) we prefer entries whose data has
    # isCompleted=True when timestamps are absent.
    with db.connection() as conn:
        rows = conn.execute(
            "SELECT id, event_id, change_type, data, device_id, device_seq, created_at "
            "FROM sync_outbox ORDER BY id"
        ).fetchall()

    best: dict[tuple[str, str], dict[str, Any]] = {}
    no_source: list[dict[str, Any]] = []

    for row in rows:
        did = str(row[4])
        seq = int(row[5])
        raw = row[3]
        try:
            parsed = json.loads(raw) if isinstance(raw, str) else raw
        except (json.JSONDecodeError, TypeError):
            parsed = raw

        if isinstance(parsed, dict):
            event_type = parsed.get("event_type", "")
            source_id = (parsed.get("source_id") or "").lower()
            occurred_at = parsed.get("occurred_at") or ""
            inner = parsed.get("data") or {}
            is_completed = inner.get("isCompleted", False) if isinstance(inner, dict) else False
        else:
            event_type = source_id = occurred_at = ""
            is_completed = False

        entry = {
            "row_id": int(row[0]),
            "event_id": str(row[1]),
            "change_type": str(row[2]),
            "data": parsed,
            "device_id": did,
            "device_seq": seq,
            "created_at": str(row[6]),
            "occurred_at": occurred_at,
            "is_completed": is_completed,
        }

        if not source_id:
            no_source.append(entry)
            continue

        key = (event_type, source_id)
        prev = best.get(key)
        if prev is None:
            best[key] = entry
        else:
            # LWW: prefer entry with later occurred_at
            # If one has occurred_at and the other doesn't, the one without
            # occurred_at is a "raw" device entry (e.g. mac) — prefer it only
            # if it marks the task as completed (completion is hard to fake stale)
            curr_has_ts = bool(occurred_at)
            prev_has_ts = bool(prev["occurred_at"])

            if curr_has_ts and prev_has_ts:
                if occurred_at > prev["occurred_at"] or (
                    occurred_at == prev["occurred_at"] and entry["row_id"] > prev["row_id"]
                ):
                    best[key] = entry
            elif curr_has_ts and not prev_has_ts:
                # prev has no timestamp (raw device entry)
                # Keep prev if it marks task completed, else use current
                if not prev["is_completed"]:
                    best[key] = entry
            elif not curr_has_ts and prev_has_ts:
                # current has no timestamp
                # Keep current if it marks task completed, else keep prev
                if is_completed:
                    best[key] = entry
            else:
                # Neither has timestamp — use row_id (insertion order)
                if entry["row_id"] > prev["row_id"]:
                    best[key] = entry

    all_entries = sorted(best.values(), key=lambda e: e["row_id"]) + no_source

    changes: list[dict[str, Any]] = []
    for e in all_entries:
        client_last = client_vector.get(e["device_id"], 0)
        if e["device_seq"] > client_last:
            changes.append({
                "event_id": e["event_id"],
                "change_type": e["change_type"],
                "data": e["data"],
                "device_id": e["device_id"],
                "device_seq": e["device_seq"],
                "created_at": e["created_at"],
            })

    return changes


def _apply_change(db: Ark, change: dict[str, Any], device_id: str) -> int:
    """
    Apply a single change to the outbox and return the assigned device_seq.
    Also applies the change to the events table via Ark API.
    """
    with db.connection() as conn:
        # Get next seq for this device
        row = conn.execute(
            "SELECT MAX(device_seq) FROM sync_outbox WHERE device_id = ?",
            (device_id,),
        ).fetchone()
        next_seq = (int(row[0]) if row and row[0] is not None else 0) + 1

        event_id = change.get("event_id") or str(uuid.uuid4())
        change_type = change.get("change_type", "create")
        data = change.get("data", {})
        data_json = json.dumps(data) if isinstance(data, dict) else str(data)

        conn.execute(
            """
            INSERT INTO sync_outbox (event_id, change_type, data, device_id, device_seq)
            VALUES (?, ?, ?, ?, ?)
            """,
            (event_id, change_type, data_json, device_id, next_seq),
        )

    # Apply to actual events table
    _apply_to_events(db, event_id, change_type, data if isinstance(data, dict) else {})

    return next_seq


def _apply_to_events(db: Ark, event_id: str, change_type: str, data: dict[str, Any]) -> None:
    """Apply a synced change to the events table.

    Upserts by source_id alone (ignoring source) so that the same logical
    object coming from different clients (delphi, delphi-web, delphi-android,
    delphi-seed) always updates a single row.
    """
    try:
        # Normalize source_id to lowercase so Mac (uppercase UUID) and
        # Android/Web (lowercase UUID) are treated as the same object.
        raw_source_id = data.get("source_id") or event_id
        source_id = raw_source_id.lower() if raw_source_id else raw_source_id

        if change_type == "delete":
            # Try to find by source_id first, fall back to event_id
            with db.connection() as conn:
                row = conn.execute(
                    "SELECT id FROM events WHERE source_id = ? AND is_deleted = 0",
                    (source_id,),
                ).fetchone()
                if row:
                    conn.execute("UPDATE events SET is_deleted = 1 WHERE id = ?", (row[0],))
                else:
                    db.delete_event(event_id)
            return

        # Upsert by source_id regardless of source
        inner_data = data.get("data", {})
        event_type = data.get("event_type", "unknown")
        category = data.get("category")
        occurred_at = data.get("occurred_at")
        summary = data.get("summary")
        source = data.get("source")
        device = data.get("device")
        tags = data.get("tags", [])

        with db.connection() as conn:
            # Check if row with this source_id already exists (any source)
            existing = conn.execute(
                "SELECT id, source FROM events WHERE LOWER(source_id) = ? AND is_deleted = 0",
                (source_id,),
            ).fetchone()

            if existing:
                # Update existing row in-place (keep original source to avoid duplicates)
                conn.execute(
                    """UPDATE events SET
                        event_type = ?, category = ?, occurred_at = COALESCE(?, occurred_at),
                        data = ?, summary = ?, device = ?, tags = ?, is_deleted = 0,
                        updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                    WHERE id = ?""",
                    (
                        event_type,
                        category,
                        occurred_at,
                        json.dumps(inner_data, ensure_ascii=False) if inner_data else "{}",
                        summary,
                        device,
                        json.dumps(tags or [], ensure_ascii=False),
                        existing[0],
                    ),
                )
            else:
                # Insert new row
                db.record_event(
                    event_type=event_type,
                    data=inner_data,
                    category=category,
                    occurred_at=occurred_at,
                    summary=summary,
                    source=source,
                    source_id=source_id,
                    device=device,
                    tags=tags,
                )
    except Exception:
        logger.exception("Failed to apply change %s/%s to events", change_type, event_id)


# ---------------------------------------------------------------------------
# Auth helper
# ---------------------------------------------------------------------------

def _check_api_key(key: Optional[str]) -> bool:
    api_key = os.environ.get("LIFE_API_KEY")
    if not api_key:
        return False
    if not key:
        return False
    return hmac.compare_digest(key, api_key)


# ---------------------------------------------------------------------------
# Heartbeat task
# ---------------------------------------------------------------------------

async def _heartbeat(ws: WebSocket, device_id: str, interval: float = 30.0) -> None:
    """Send ping every `interval` seconds. Exits on any error."""
    try:
        while True:
            await asyncio.sleep(interval)
            await ws.send_json({"type": "ping"})
    except Exception:
        logger.debug("Heartbeat stopped for %s", device_id, exc_info=True)


# ---------------------------------------------------------------------------
# WebSocket endpoint
# ---------------------------------------------------------------------------

_db_ref: Ark | None = None


def init_sync(db: Ark) -> None:
    """Initialize sync module with the Ark database instance."""
    global _db_ref
    _db_ref = db
    _ensure_sync_tables(db)


def _get_db() -> Ark:
    if _db_ref is None:
        raise RuntimeError("sync_ws not initialized — call init_sync(db) first")
    return _db_ref


@router.websocket("/ws/sync")
async def websocket_sync(websocket: WebSocket, key: Optional[str] = None) -> None:
    # Auth
    if not _check_api_key(key):
        await websocket.close(code=4003, reason="unauthorized")
        return

    await websocket.accept()

    device_id: Optional[str] = None
    heartbeat_task: asyncio.Task[None] | None = None

    try:
        db = _get_db()
        # ---- Wait for sync_start (with timeout to prevent idle connections) ----
        logger.info("Waiting for sync_start from %s", websocket.client)
        try:
            raw = await asyncio.wait_for(websocket.receive_text(), timeout=30.0)
        except asyncio.TimeoutError:
            logger.warning("sync_start timeout from %s", websocket.client)
            await websocket.close(code=4000, reason="sync_start timeout")
            return
        logger.info("Received sync_start raw=%r", raw[:200] if raw else None)
        msg = json.loads(raw)

        if msg.get("type") != "sync_start":
            await websocket.send_json({"type": "error", "message": "Expected sync_start"})
            await websocket.close(code=4000)
            return

        device_id = msg.get("device_id", "")
        device_name = msg.get("device_name", device_id)
        platform = msg.get("platform", "unknown")
        client_vector: dict[str, int] = msg.get("vector", {})

        if not device_id:
            await websocket.send_json({"type": "error", "message": "device_id required"})
            await websocket.close(code=4000)
            return

        # Register device & connection
        _register_device(db, device_id, device_name, platform)
        await manager.connect(device_id, websocket, device_name, platform)
        logger.info("Device connected: %s (%s/%s), vector=%s, total_connections=%d",
                     device_id, device_name, platform, client_vector, len(manager.connections))

        # Send changes the client hasn't seen
        changes = _get_changes_since(db, client_vector)
        logger.info("Sending %d missed changes to %s", len(changes), device_id)
        is_full_sync = not bool(client_vector)
        task_count = sum(1 for c in changes if isinstance(c.get("data"), dict) and c["data"].get("event_type") == "task")
        with db.connection() as _conn:
            server_epoch = _conn.execute(
                "SELECT value FROM sync_meta WHERE key='server_epoch'"
            ).fetchone()[0]
        await websocket.send_json({
            "type": "sync_changes",
            "changes": changes,
            "is_full_sync": is_full_sync,
            "task_count": task_count,
            "server_epoch": server_epoch,
        })

        # Start heartbeat
        heartbeat_task = asyncio.create_task(_heartbeat(websocket, device_id))

        # ---- Main loop: receive messages ----
        while True:
            raw = await websocket.receive_text()
            msg = json.loads(raw)
            msg_type = msg.get("type")

            if msg_type == "sync_changes":
                # Client sends batch of changes it has that server doesn't
                incoming_changes: list[dict[str, Any]] = msg.get("changes", [])
                logger.info("Received %d sync_changes from %s", len(incoming_changes), device_id)
                for ch in incoming_changes:
                    seq = _apply_change(db, ch, device_id)
                    logger.info("Applied change %s/%s from %s, seq=%d, broadcasting to %d peers",
                                ch.get("event_id"), ch.get("change_type"), device_id, seq,
                                len(manager.connections) - 1)
                    broadcast_msg = {
                        "type": "change",
                        "event_id": ch.get("event_id"),
                        "change_type": ch.get("change_type"),
                        "data": ch.get("data"),
                        "device_id": device_id,
                        "device_seq": seq,
                    }
                    # Broadcast to other connected clients
                    await manager.broadcast(broadcast_msg, exclude=device_id)
                    # Relay to outbound peers
                    await _relay_to_peers(ch, device_id, seq)
                # Acknowledge
                await websocket.send_json({
                    "type": "sync_ack",
                    "applied": len(incoming_changes),
                })

            elif msg_type == "change":
                # Single realtime change
                seq = _apply_change(db, msg, device_id)
                logger.info("Realtime change %s/%s from %s, seq=%d, broadcasting to %d peers",
                            msg.get("event_id"), msg.get("change_type"), device_id, seq,
                            len(manager.connections) - 1)
                await manager.broadcast(
                    {
                        "type": "change",
                        "event_id": msg.get("event_id"),
                        "change_type": msg.get("change_type"),
                        "data": msg.get("data"),
                        "device_id": device_id,
                        "device_seq": seq,
                    },
                    exclude=device_id,
                )
                # Relay to outbound peers
                await _relay_to_peers(msg, device_id, seq)
                await websocket.send_json({
                    "type": "change_ack",
                    "event_id": msg.get("event_id"),
                    "device_seq": seq,
                })

            elif msg_type == "pong":
                # Response to our ping — do nothing
                pass

            else:
                await websocket.send_json({
                    "type": "error",
                    "message": f"Unknown message type: {msg_type}",
                })

    except WebSocketDisconnect:
        logger.info("Device %s disconnected (WebSocketDisconnect)", device_id)
    except json.JSONDecodeError as e:
        logger.warning("Invalid JSON from device %s: %s", device_id, e)
    except Exception as e:
        logger.exception("WebSocket error for device %s: %s", device_id, e)
    finally:
        if heartbeat_task:
            heartbeat_task.cancel()
        if device_id:
            manager.disconnect(device_id)


async def _relay_to_peers(change: dict[str, Any], origin_device: str, seq: int) -> None:
    """Relay a change to all outbound peer connectors."""
    try:
        from server.peer_connector import peer_connector
        from core.peer_protocol import PeerChange
        import os, platform as _platform
        server_id = os.environ.get("ARK_DEVICE_ID", f"ark-server-{_platform.node()}")
        peer_change = PeerChange(
            event_id=change.get("event_id", ""),
            change_type=change.get("change_type", "create"),
            data=change.get("data", {}),
            origin_device=origin_device,
            origin_seq=seq,
            hlc="",
            hop_path=[server_id],
        )
        await peer_connector.relay_change(peer_change)
    except Exception:
        logger.debug("relay_to_peers failed", exc_info=True)
