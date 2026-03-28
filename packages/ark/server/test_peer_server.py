"""Tests for the P2P WebSocket peer sync endpoint."""

import json
import os
import tempfile

import pytest
from starlette.testclient import TestClient

# Set env vars before importing the app
os.environ.setdefault("LIFE_API_KEY", "test-secret-key")
os.environ.setdefault("ARK_MDNS", "0")

from core.peer_protocol import (
    PeerChange,
    PeerHello,
    compute_auth_hmac,
    compute_mesh_id,
    generate_nonce,
    make_message,
)


@pytest.fixture(autouse=True)
def _isolate_db(tmp_path, monkeypatch):
    """Use a temporary database for each test."""
    db_path = str(tmp_path / "test.db")
    monkeypatch.setenv("LIFE_DB_PATH", db_path)
    monkeypatch.setenv("LIFE_API_KEY", "test-secret-key")
    monkeypatch.setenv("ARK_MDNS", "0")

    # Re-initialize the app's db and sync modules with the temp path
    from core.ark import Ark
    import server.app as app_mod
    import server.sync_ws as sync_ws_mod
    import server.peer_server as peer_mod

    db = Ark(db_path, create=True)
    app_mod.db = db
    sync_ws_mod.init_sync(db)
    peer_mod.init_peer_sync(db)

    yield db


@pytest.fixture
def client():
    from server.app import app
    return TestClient(app)


MESH_SECRET = "test-secret-key"


def _make_hello(device_id: str = "test-device-1", device_name: str = "Test Device") -> str:
    hello = PeerHello.create(
        device_id=device_id,
        device_name=device_name,
        platform="test",
        mesh_secret=MESH_SECRET,
    )
    return make_message("peer_hello", **hello.to_dict())


def _make_bad_hello() -> str:
    """Create a peer_hello with an invalid HMAC."""
    nonce = generate_nonce()
    return make_message(
        "peer_hello",
        protocol_version=2,
        device_id="bad-device",
        device_name="Bad Device",
        platform="test",
        mesh_id=compute_mesh_id(MESH_SECRET),
        nonce=nonce,
        auth_hmac="0000dead0000beef0000dead0000beef0000dead0000beef0000dead0000beef",
    )


def test_peer_hello_valid_auth(client):
    """Valid HMAC is accepted, returns peer_hello_ack with ok=True."""
    with client.websocket_connect("/peer/sync") as ws:
        ws.send_text(_make_hello())
        raw = ws.receive_text()
        msg = json.loads(raw)

        assert msg["type"] == "peer_hello_ack"
        assert msg["ok"] is True
        assert msg["error"] is None
        assert "device_id" in msg
        assert "nonce" in msg
        assert "auth_hmac" in msg


def test_peer_hello_invalid_auth(client):
    """Bad HMAC is rejected, connection closed."""
    with pytest.raises(Exception):
        with client.websocket_connect("/peer/sync") as ws:
            ws.send_text(_make_bad_hello())
            raw = ws.receive_text()
            msg = json.loads(raw)
            assert msg["type"] == "peer_hello_ack"
            assert msg["ok"] is False
            assert msg["error"] == "auth_failed"
            # Connection should close; next receive should fail
            ws.receive_text()


def test_peer_hello_wrong_message_type(client):
    """Non peer_hello first message is rejected."""
    with pytest.raises(Exception):
        with client.websocket_connect("/peer/sync") as ws:
            ws.send_text(make_message("sync_start", device_id="x", vector={}))
            # Server should close with 4001
            ws.receive_text()


def test_peer_sync_exchange(client):
    """Two peers exchange changes after auth."""
    # Peer 1 connects and sends a change
    with client.websocket_connect("/peer/sync") as ws1:
        ws1.send_text(_make_hello("peer-1", "Peer 1"))
        ack1 = json.loads(ws1.receive_text())
        assert ack1["ok"] is True

        # Send sync_start to establish vectors
        ws1.send_text(make_message("sync_start", vector={}))
        sync_resp = json.loads(ws1.receive_text())
        assert sync_resp["type"] == "sync_changes"

        # Peer 1 sends a change
        change = PeerChange(
            event_id="evt-100",
            change_type="create",
            data={"event_type": "task", "summary": "Test task"},
            origin_device="peer-1",
            origin_seq=1,
            hlc="2026-03-28T14:30:00.000000Z:000001:peer-1",
            hop_path=["peer-1"],
        )
        ws1.send_text(make_message("peer_change", **change.to_dict()))
        change_ack = json.loads(ws1.receive_text())
        assert change_ack["type"] == "peer_change_ack"
        assert change_ack["event_id"] == "evt-100"

    # Peer 2 connects and should see the change from peer 1
    with client.websocket_connect("/peer/sync") as ws2:
        ws2.send_text(_make_hello("peer-2", "Peer 2"))
        ack2 = json.loads(ws2.receive_text())
        assert ack2["ok"] is True

        # Request changes with empty vector
        ws2.send_text(make_message("sync_start", vector={}))
        sync_resp2 = json.loads(ws2.receive_text())
        assert sync_resp2["type"] == "sync_changes"
        changes = sync_resp2.get("changes", [])
        # Should contain the change from peer-1
        event_ids = [c["event_id"] for c in changes]
        assert "evt-100" in event_ids


def test_loop_prevention(client):
    """Change with hop_path containing receiver is not re-sent."""
    from server.peer_server import peer_manager

    with client.websocket_connect("/peer/sync") as ws1:
        ws1.send_text(_make_hello("peer-A", "Peer A"))
        ack = json.loads(ws1.receive_text())
        assert ack["ok"] is True

        # Simulate a change that already passed through this server
        from server.peer_server import _server_device_id
        server_id = _server_device_id()

        change = PeerChange(
            event_id="evt-loop",
            change_type="create",
            data={"event_type": "task", "summary": "Loop test"},
            origin_device="peer-X",
            origin_seq=1,
            hlc="2026-03-28T14:30:00.000000Z:000001:peer-X",
            hop_path=["peer-X", server_id],  # already visited this server
        )
        ws1.send_text(make_message("peer_change", **change.to_dict()))

        # The server should detect itself in hop_path and skip processing.
        # Since it skips, there is no peer_change_ack sent back.
        # Send a sync_start to verify the connection is still alive.
        ws1.send_text(make_message("sync_start", vector={}))
        resp = json.loads(ws1.receive_text())
        assert resp["type"] == "sync_changes"
        # The looped change should NOT appear in changes (it was skipped entirely)
        event_ids = [c["event_id"] for c in resp.get("changes", [])]
        assert "evt-loop" not in event_ids
