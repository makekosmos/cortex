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
                pass
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
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
        """)


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
        conn.commit()


def _get_server_vector(db: Ark) -> dict[str, int]:
    """Return the server's view: max device_seq per device_id from outbox."""
    with db.connection() as conn:
        rows = conn.execute(
            "SELECT device_id, MAX(device_seq) as max_seq FROM sync_outbox GROUP BY device_id"
        ).fetchall()
    return {str(row[0]): int(row[1]) for row in rows}


def _get_changes_since(db: Ark, client_vector: dict[str, int]) -> list[dict[str, Any]]:
    """Return outbox entries the client hasn't seen based on its vector."""
    changes: list[dict[str, Any]] = []
    with db.connection() as conn:
        if not client_vector:
            # Client has nothing — send everything
            rows = conn.execute(
                "SELECT id, event_id, change_type, data, device_id, device_seq, created_at "
                "FROM sync_outbox ORDER BY id"
            ).fetchall()
        else:
            # Build a query for rows the client hasn't seen
            # For known devices: seq > client's last_seq
            # For unknown devices: all rows
            rows = conn.execute(
                "SELECT id, event_id, change_type, data, device_id, device_seq, created_at "
                "FROM sync_outbox ORDER BY id"
            ).fetchall()

    for row in rows:
        did = str(row[4])
        seq = int(row[5])
        client_last = client_vector.get(did, 0)
        if seq > client_last:
            data = row[3]
            try:
                parsed_data = json.loads(data)
            except (json.JSONDecodeError, TypeError):
                parsed_data = data
            changes.append({
                "event_id": str(row[1]),
                "change_type": str(row[2]),
                "data": parsed_data,
                "device_id": did,
                "device_seq": seq,
                "created_at": str(row[6]),
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
        conn.commit()

    # Apply to actual events table
    _apply_to_events(db, event_id, change_type, data if isinstance(data, dict) else {})

    return next_seq


def _apply_to_events(db: Ark, event_id: str, change_type: str, data: dict[str, Any]) -> None:
    """Apply a synced change to the events table."""
    try:
        if change_type == "create":
            db.record_event(
                event_type=data.get("event_type", "unknown"),
                data=data.get("data", {}),
                category=data.get("category"),
                occurred_at=data.get("occurred_at"),
                summary=data.get("summary"),
                source=data.get("source"),
                source_id=data.get("source_id") or event_id,
                device=data.get("device"),
                tags=data.get("tags", []),
            )
        elif change_type == "update":
            # For updates, we re-record (upsert via source_id)
            db.record_event(
                event_type=data.get("event_type", "unknown"),
                data=data.get("data", {}),
                category=data.get("category"),
                occurred_at=data.get("occurred_at"),
                summary=data.get("summary"),
                source=data.get("source"),
                source_id=data.get("source_id") or event_id,
                device=data.get("device"),
                tags=data.get("tags", []),
            )
        elif change_type == "delete":
            db.delete_event(event_id)
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
        pass


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

    db = _get_db()
    device_id: Optional[str] = None
    heartbeat_task: asyncio.Task[None] | None = None

    try:
        # ---- Wait for sync_start ----
        raw = await websocket.receive_text()
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

        # Send changes the client hasn't seen
        changes = _get_changes_since(db, client_vector)
        await websocket.send_json({"type": "sync_changes", "changes": changes})

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
                for ch in incoming_changes:
                    seq = _apply_change(db, ch, device_id)
                    # Broadcast to other connected clients
                    await manager.broadcast(
                        {
                            "type": "change",
                            "event_id": ch.get("event_id"),
                            "change_type": ch.get("change_type"),
                            "data": ch.get("data"),
                            "device_id": device_id,
                            "device_seq": seq,
                        },
                        exclude=device_id,
                    )
                # Acknowledge
                await websocket.send_json({
                    "type": "sync_ack",
                    "applied": len(incoming_changes),
                })

            elif msg_type == "change":
                # Single realtime change
                seq = _apply_change(db, msg, device_id)
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
        logger.info("Device %s disconnected", device_id)
    except json.JSONDecodeError:
        logger.warning("Invalid JSON from device %s", device_id)
    except Exception:
        logger.exception("WebSocket error for device %s", device_id)
    finally:
        if heartbeat_task:
            heartbeat_task.cancel()
        if device_id:
            manager.disconnect(device_id)
