"""Outbound peer connector — connects this Ark instance to remote peers.

Usage: set ARK_PEER_URLS=ws://vps.example.com:8000 (comma-separated).
The connector is started from app.py lifespan if ARK_PEER_URLS is set.
"""

from __future__ import annotations

import asyncio
import logging
import os
import platform
from typing import Any, Optional

import websockets
from websockets.exceptions import ConnectionClosed, WebSocketException

from core.peer_protocol import (
    PeerChange,
    PeerHello,
    PeerHelloAck,
    make_message,
    parse_message,
    verify_auth_hmac,
)
from core.hlc import HLC

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Config helpers
# ---------------------------------------------------------------------------

_INITIAL_BACKOFF = 2.0
_MAX_BACKOFF = 60.0


def _get_mesh_secret() -> str:
    secret = os.environ.get("LIFE_API_KEY", "")
    if not secret:
        raise RuntimeError("LIFE_API_KEY not set — cannot authenticate peers")
    return secret


def _device_id() -> str:
    return os.environ.get("ARK_DEVICE_ID", f"ark-server-{platform.node()}")


def _device_name() -> str:
    return os.environ.get("ARK_DEVICE_NAME", platform.node() or "ark-server")


def _peer_urls() -> list[str]:
    raw = os.environ.get("ARK_PEER_URLS", "").strip()
    if not raw:
        return []
    return [u.strip() for u in raw.split(",") if u.strip()]


# ---------------------------------------------------------------------------
# Single peer connection
# ---------------------------------------------------------------------------


class _PeerSession:
    """Manages a single outbound WebSocket connection to a remote Ark peer."""

    def __init__(self, peer_url: str, connector: "PeerConnector") -> None:
        self.peer_url = peer_url
        self.connector = connector
        self._ws: Optional[Any] = None
        self._device_id: Optional[str] = None

    async def run(self) -> None:
        """Connect, handshake, sync, and relay. Reconnect on failure."""
        backoff = _INITIAL_BACKOFF
        while not self.connector._stopped:
            try:
                await self._connect_and_sync()
                # Clean disconnect — reset backoff
                backoff = _INITIAL_BACKOFF
            except asyncio.CancelledError:
                logger.info("Peer session %s cancelled", self.peer_url)
                return
            except (ConnectionClosed, WebSocketException, OSError) as exc:
                logger.warning(
                    "Peer %s disconnected: %s. Reconnecting in %.0fs",
                    self.peer_url,
                    exc,
                    backoff,
                )
            except Exception:
                logger.exception(
                    "Unexpected error for peer %s. Reconnecting in %.0fs",
                    self.peer_url,
                    backoff,
                )

            if self.connector._stopped:
                break
            await asyncio.sleep(backoff)
            backoff = min(backoff * 2, _MAX_BACKOFF)

    async def _connect_and_sync(self) -> None:
        ws_url = f"{self.peer_url.rstrip('/')}/peer/sync"
        logger.info("Connecting to peer %s", ws_url)

        async with websockets.connect(ws_url, ping_interval=None) as ws:
            self._ws = ws
            try:
                await self._handshake(ws)
                await self._sync_loop(ws)
            finally:
                self._ws = None
                if self._device_id:
                    self.connector._sessions.pop(self._device_id, None)
                    self._device_id = None

    async def _handshake(self, ws: Any) -> None:
        mesh_secret = _get_mesh_secret()

        # Step 1: send peer_hello
        hello = PeerHello.create(
            device_id=_device_id(),
            device_name=_device_name(),
            platform="server",
            mesh_secret=mesh_secret,
        )
        await ws.send(make_message("peer_hello", **hello.to_dict()))

        # Step 2: receive peer_hello_ack (with timeout)
        try:
            raw = await asyncio.wait_for(ws.recv(), timeout=30.0)
        except asyncio.TimeoutError:
            raise ConnectionError("peer_hello_ack timeout")

        msg = parse_message(raw)
        if msg.get("type") != "peer_hello_ack":
            raise ConnectionError(f"Expected peer_hello_ack, got {msg.get('type')!r}")

        if not msg.get("ok"):
            raise ConnectionError(f"Peer rejected hello: {msg.get('error')}")

        # Step 3: verify the ack's HMAC (mutual auth)
        if not verify_auth_hmac(mesh_secret, msg["nonce"], msg["auth_hmac"]):
            raise ConnectionError("peer_hello_ack HMAC verification failed")

        device_id = msg["device_id"]
        assert isinstance(device_id, str), "device_id must be a string"
        self._device_id = device_id
        logger.info(
            "Handshake OK with %s (%s/%s)",
            self._device_id,
            msg.get("device_name"),
            msg.get("platform"),
        )

        # Register this session for relay
        self.connector._sessions[self._device_id] = self

    async def _sync_loop(self, ws: Any) -> None:
        from server.sync_ws import _get_changes_since, _apply_change, _ensure_sync_tables
        from server.peer_server import peer_manager

        db = self.connector._db
        _ensure_sync_tables(db)

        # Step 4: send sync_start with our version vector
        from server.sync_ws import _get_server_vector
        vector = _get_server_vector(db)
        await ws.send(make_message("sync_start", vector=vector))
        logger.info("Sent sync_start with vector=%s to %s", vector, self._device_id)

        server_hlc = HLC.now(_device_id())

        async for raw in ws:
            msg = parse_message(raw)
            msg_type = msg.get("type")

            if msg_type == "sync_changes":
                # Step 5: apply missed changes from peer
                changes = msg.get("changes", [])
                logger.info(
                    "Received %d sync_changes from peer %s",
                    len(changes),
                    self._device_id,
                )
                for ch in changes:
                    origin = str(ch.get("origin_device") or ch.get("device_id") or self._device_id or "unknown")
                    try:
                        _apply_change(db, {
                            "event_id": ch.get("event_id"),
                            "change_type": ch.get("change_type"),
                            "data": ch.get("data"),
                        }, origin)
                    except Exception:
                        logger.exception(
                            "Failed to apply sync_change %s from peer %s",
                            ch.get("event_id"),
                            self._device_id,
                        )

            elif msg_type == "peer_change":
                # Step 6: relay incoming peer_change into local DB + broadcast
                change = PeerChange.from_dict(msg)

                # Loop prevention: skip if we're already in hop_path
                if _device_id() in change.hop_path:
                    continue

                try:
                    _apply_change(db, {
                        "event_id": change.event_id,
                        "change_type": change.change_type,
                        "data": change.data,
                    }, change.origin_device)
                except Exception:
                    logger.exception(
                        "Failed to apply peer_change %s from peer %s",
                        change.event_id,
                        self._device_id,
                    )
                    continue

                # Merge HLC
                if change.hlc:
                    try:
                        remote_hlc = HLC.from_str(change.hlc)
                        server_hlc = server_hlc.merge(remote_hlc)
                    except Exception:
                        pass

                # Broadcast to local peers (excluding the source)
                forwarded = PeerChange(
                    event_id=change.event_id,
                    change_type=change.change_type,
                    data=change.data,
                    origin_device=change.origin_device,
                    origin_seq=change.origin_seq,
                    hlc=str(server_hlc.tick()),
                    hop_path=change.hop_path + [_device_id()],
                )
                await peer_manager.broadcast_change(forwarded, exclude_device=self._device_id or "")

                # Acknowledge
                await ws.send(make_message(
                    "peer_change_ack",
                    event_id=change.event_id,
                ))

            elif msg_type == "ping":
                await ws.send(make_message("pong"))

            elif msg_type == "pong":
                pass

            else:
                logger.debug(
                    "Unknown message type %r from peer %s", msg_type, self._device_id
                )

    async def send_change(self, change: PeerChange) -> None:
        """Send a peer_change to this peer. Called from PeerConnector.relay_change()."""
        ws = self._ws
        if ws is None:
            return
        # Skip if we're already in hop_path (loop prevention)
        if _device_id() in change.hop_path:
            return
        try:
            await ws.send(make_message("peer_change", **change.to_dict()))
        except Exception:
            logger.debug(
                "Failed to relay change %s to peer %s", change.event_id, self.peer_url
            )


# ---------------------------------------------------------------------------
# PeerConnector — public API
# ---------------------------------------------------------------------------


class PeerConnector:
    """
    Manages outbound connections to all ARK_PEER_URLS peers.

    Usage (from app.py lifespan):
        connector = PeerConnector()
        connector.start(db)
        ...
        connector.stop()

    When a local client sends a change (via sync_ws), call:
        connector.relay_change(change)
    """

    def __init__(self) -> None:
        self._db: Any = None
        self._stopped = False
        self._tasks: list[asyncio.Task] = []
        # device_id -> _PeerSession (for relay)
        self._sessions: dict[str, "_PeerSession"] = {}

    def start(self, db: Any) -> None:
        """Start outbound connections to all peers listed in ARK_PEER_URLS."""
        self._db = db
        self._stopped = False
        urls = _peer_urls()
        if not urls:
            logger.info("ARK_PEER_URLS not set — peer connector inactive")
            return
        for url in urls:
            session = _PeerSession(peer_url=url, connector=self)
            task = asyncio.create_task(session.run(), name=f"peer-connector:{url}")
            self._tasks.append(task)
        logger.info("PeerConnector started, connecting to: %s", urls)

    def stop(self) -> None:
        """Disconnect all outbound peers."""
        self._stopped = True
        for task in self._tasks:
            task.cancel()
        self._tasks.clear()
        self._sessions.clear()
        logger.info("PeerConnector stopped")

    async def relay_change(self, change: PeerChange) -> None:
        """
        Send a change to all connected outbound peers.
        Called from sync_ws.py when a local client submits a change.
        """
        for session in list(self._sessions.values()):
            await session.send_change(change)


# Module-level singleton — imported by app.py
peer_connector = PeerConnector()
