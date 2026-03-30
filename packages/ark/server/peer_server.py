"""P2P WebSocket endpoint for peer-to-peer sync."""

from __future__ import annotations

import asyncio
import json
import logging
import os
import platform
from dataclasses import asdict
from typing import Any, Optional

from fastapi import APIRouter, WebSocket, WebSocketDisconnect

from core.peer_protocol import (
    PeerChange,
    PeerHelloAck,
    compute_mesh_id,
    make_message,
    parse_message,
    verify_auth_hmac,
)
from core.hlc import HLC

logger = logging.getLogger(__name__)

router = APIRouter()


# ---------------------------------------------------------------------------
# Config helpers
# ---------------------------------------------------------------------------

def _get_mesh_secret() -> str:
    """Return the mesh secret (== LIFE_API_KEY)."""
    secret = os.environ.get("LIFE_API_KEY", "")
    if not secret:
        raise RuntimeError("LIFE_API_KEY not set — cannot authenticate peers")
    return secret


def _server_device_id() -> str:
    return os.environ.get("ARK_DEVICE_ID", f"ark-server-{platform.node()}")


def _server_device_name() -> str:
    return os.environ.get("ARK_DEVICE_NAME", platform.node() or "ark-server")


# ---------------------------------------------------------------------------
# Connection manager
# ---------------------------------------------------------------------------


class PeerConnectionManager:
    """Manages active peer WebSocket connections."""

    def __init__(self) -> None:
        self.connections: dict[str, WebSocket] = {}  # device_id -> ws
        self.peer_vectors: dict[str, dict[str, int]] = {}  # device_id -> version vector
        self._lock = asyncio.Lock()

    async def add_peer(self, device_id: str, ws: WebSocket) -> None:
        async with self._lock:
            # Close previous connection from same device
            old_ws = self.connections.get(device_id)
            if old_ws is not None:
                try:
                    await old_ws.close(code=4001, reason="replaced")
                except Exception:
                    pass
            self.connections[device_id] = ws

    async def remove_peer(self, device_id: str) -> None:
        async with self._lock:
            self.connections.pop(device_id, None)
            self.peer_vectors.pop(device_id, None)

    async def broadcast_change(self, change: PeerChange, exclude_device: str) -> None:
        """Send a change to all connected peers, skipping the sender and peers in hop_path."""
        msg = make_message("peer_change", **change.to_dict())
        async with self._lock:
            targets = [
                (did, ws) for did, ws in self.connections.items()
                if did != exclude_device and did not in change.hop_path
            ]
        # Send outside lock to avoid holding it during I/O
        dead: list[str] = []
        for did, ws in targets:
            try:
                await ws.send_text(msg)
            except Exception:
                dead.append(did)
        if dead:
            async with self._lock:
                for did in dead:
                    self.connections.pop(did, None)
                    self.peer_vectors.pop(did, None)


peer_manager = PeerConnectionManager()


# ---------------------------------------------------------------------------
# Sync helpers — reuse the sync_ws infrastructure
# ---------------------------------------------------------------------------

_db_ref = None


def init_peer_sync(db: Any) -> None:
    """Initialize peer sync with the Ark database instance."""
    global _db_ref  # noqa: PLW0603
    _db_ref = db


def _get_db():
    if _db_ref is None:
        raise RuntimeError("peer_server not initialized — call init_peer_sync(db) first")
    return _db_ref


# ---------------------------------------------------------------------------
# Heartbeat
# ---------------------------------------------------------------------------

async def _heartbeat(ws: WebSocket, interval: float = 30.0) -> None:
    try:
        while True:
            await asyncio.sleep(interval)
            await ws.send_text(make_message("ping"))
    except Exception:
        pass


# ---------------------------------------------------------------------------
# WebSocket endpoint
# ---------------------------------------------------------------------------


@router.websocket("/peer/sync")
async def peer_sync(ws: WebSocket) -> None:
    """
    P2P sync WebSocket endpoint.

    Flow:
    1. Accept connection
    2. Receive peer_hello, verify HMAC
    3. Send peer_hello_ack with our HMAC
    4. Exchange sync_start with version vectors
    5. Send/receive changes with origin tracking and loop prevention
    """
    await ws.accept()

    device_id: Optional[str] = None
    heartbeat_task: Optional[asyncio.Task] = None

    try:
        # ---- Step 1: Receive peer_hello (with timeout) ----
        try:
            raw = await asyncio.wait_for(ws.receive_text(), timeout=30.0)
        except asyncio.TimeoutError:
            await ws.close(4000, "peer_hello timeout")
            return

        msg = parse_message(raw)

        if msg.get("type") != "peer_hello":
            await ws.close(4001, "Expected peer_hello")
            return

        # ---- Step 2: Verify HMAC ----
        mesh_secret = _get_mesh_secret()
        expected_mesh_id = compute_mesh_id(mesh_secret)

        if msg.get("mesh_id") != expected_mesh_id:
            await ws.send_text(make_message(
                "peer_hello_ack", ok=False, error="mesh_id_mismatch",
            ))
            await ws.close(4003, "Mesh ID mismatch")
            return

        if not verify_auth_hmac(mesh_secret, msg["nonce"], msg["auth_hmac"]):
            await ws.send_text(make_message(
                "peer_hello_ack", ok=False, error="auth_failed",
            ))
            await ws.close(4003, "Authentication failed")
            return

        device_id = str(msg["device_id"])
        peer_name = str(msg.get("device_name", device_id))
        peer_platform = str(msg.get("platform", "unknown"))

        # ---- Step 3: Send our hello_ack ----
        ack = PeerHelloAck.create(
            ok=True,
            device_id=_server_device_id(),
            device_name=_server_device_name(),
            platform="server",
            mesh_secret=mesh_secret,
        )
        await ws.send_text(make_message("peer_hello_ack", **ack.to_dict()))

        # Register peer
        await peer_manager.add_peer(device_id, ws)
        logger.info(
            "Peer authenticated: %s (%s/%s), total_peers=%d",
            device_id, peer_name, peer_platform, len(peer_manager.connections),
        )

        # Start heartbeat
        heartbeat_task = asyncio.create_task(_heartbeat(ws))

        # ---- Step 4-5: Sync loop ----
        server_hlc = HLC.now(_server_device_id())

        while True:
            raw = await ws.receive_text()
            msg = parse_message(raw)
            msg_type = msg.get("type")

            if msg_type == "sync_start":
                # Peer sends its version vector; we respond with missed changes
                client_vector: dict[str, int] = msg.get("vector", {})
                peer_manager.peer_vectors[device_id] = client_vector

                # Use the sync_ws helpers to get missed changes
                from server.sync_ws import _get_changes_since, _ensure_sync_tables
                db = _get_db()
                _ensure_sync_tables(db)
                changes = _get_changes_since(db, client_vector)

                # Wrap as PeerChange with hop_path for loop prevention
                peer_changes = []
                for ch in changes:
                    pc = PeerChange(
                        event_id=ch["event_id"],
                        change_type=ch["change_type"],
                        data=ch["data"],
                        origin_device=ch.get("device_id", _server_device_id()),
                        origin_seq=ch.get("device_seq", 0),
                        hlc=str(server_hlc.tick()),
                        hop_path=[_server_device_id()],
                    )
                    peer_changes.append(pc.to_dict())

                await ws.send_text(make_message(
                    "sync_changes",
                    changes=peer_changes,
                ))

            elif msg_type == "peer_change":
                # Receive a change from the peer
                change = PeerChange.from_dict(msg)

                # Loop prevention: if we're already in hop_path, skip
                if _server_device_id() in change.hop_path:
                    continue

                # Apply the change
                from server.sync_ws import _apply_change, _ensure_sync_tables
                db = _get_db()
                _ensure_sync_tables(db)
                seq = _apply_change(db, {
                    "event_id": change.event_id,
                    "change_type": change.change_type,
                    "data": change.data,
                }, change.origin_device)

                # Merge HLC
                if change.hlc:
                    remote_hlc = HLC.from_str(change.hlc)
                    server_hlc = server_hlc.merge(remote_hlc)

                # Add ourselves to hop_path and broadcast to other peers
                forwarded = PeerChange(
                    event_id=change.event_id,
                    change_type=change.change_type,
                    data=change.data,
                    origin_device=change.origin_device,
                    origin_seq=change.origin_seq,
                    hlc=str(server_hlc.tick()),
                    hop_path=change.hop_path + [_server_device_id()],
                )
                await peer_manager.broadcast_change(forwarded, exclude_device=device_id)

                # Acknowledge
                await ws.send_text(make_message(
                    "peer_change_ack",
                    event_id=change.event_id,
                    device_seq=seq,
                ))

            elif msg_type == "pong":
                pass

            else:
                await ws.send_text(make_message(
                    "error",
                    message=f"Unknown message type: {msg_type}",
                ))

    except WebSocketDisconnect:
        logger.info("Peer %s disconnected", device_id)
    except Exception:
        logger.exception("Peer WebSocket error for %s", device_id)
    finally:
        if heartbeat_task:
            heartbeat_task.cancel()
        if device_id:
            await peer_manager.remove_peer(device_id)
