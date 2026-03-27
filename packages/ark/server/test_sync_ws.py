"""
Tests for the WebSocket sync endpoint.

Run:
    LIFE_API_KEY=test-key pytest server/test_sync_ws.py -v
"""

from __future__ import annotations

import json
import os
import tempfile

import pytest
from fastapi.testclient import TestClient

# Set env before importing app
os.environ.setdefault("LIFE_API_KEY", "test-key")


@pytest.fixture()
def client(tmp_path):
    """Create a TestClient with a temporary database."""
    db_path = str(tmp_path / "test.db")
    os.environ["LIFE_DB_PATH"] = db_path

    # Re-import to pick up the new DB path.
    # We reload the modules so each test gets a fresh DB.
    import importlib
    import server.sync_ws as sync_ws_mod
    import server.app as app_mod

    importlib.reload(sync_ws_mod)
    importlib.reload(app_mod)

    yield TestClient(app_mod.app)


API_KEY = os.environ.get("LIFE_API_KEY", "test-key")


class TestSyncWebSocket:
    def test_reject_without_key(self, client: TestClient):
        """Connection without API key should be rejected."""
        from starlette.websockets import WebSocketDisconnect

        with pytest.raises(WebSocketDisconnect):
            with client.websocket_connect("/ws/sync") as ws:
                pytest.fail("Should have been rejected")  # pragma: no cover

    def test_connect_and_sync_start(self, client: TestClient):
        """Client can connect, send sync_start, and receive sync_changes."""
        with client.websocket_connect(f"/ws/sync?key={API_KEY}") as ws:
            ws.send_json({
                "type": "sync_start",
                "device_id": "test-device-1",
                "device_name": "Test Device",
                "platform": "test",
                "vector": {},
            })

            resp = ws.receive_json()
            assert resp["type"] == "sync_changes"
            assert isinstance(resp["changes"], list)
            # Fresh DB — no changes
            assert len(resp["changes"]) == 0

    def test_send_change_and_ack(self, client: TestClient):
        """Client sends a realtime change and gets an ack."""
        with client.websocket_connect(f"/ws/sync?key={API_KEY}") as ws:
            # Initial handshake
            ws.send_json({
                "type": "sync_start",
                "device_id": "test-device-2",
                "device_name": "Test Device 2",
                "platform": "test",
                "vector": {},
            })
            ws.receive_json()  # sync_changes

            # Send a change
            ws.send_json({
                "type": "change",
                "event_id": "evt-001",
                "change_type": "create",
                "data": {
                    "event_type": "task",
                    "data": {"title": "Buy milk"},
                    "summary": "Buy milk",
                },
            })

            ack = ws.receive_json()
            assert ack["type"] == "change_ack"
            assert ack["event_id"] == "evt-001"
            assert ack["device_seq"] == 1

    def test_sync_changes_batch(self, client: TestClient):
        """Client sends a batch of changes via sync_changes."""
        with client.websocket_connect(f"/ws/sync?key={API_KEY}") as ws:
            ws.send_json({
                "type": "sync_start",
                "device_id": "test-device-3",
                "device_name": "Test Device 3",
                "platform": "test",
                "vector": {},
            })
            ws.receive_json()  # sync_changes

            ws.send_json({
                "type": "sync_changes",
                "changes": [
                    {
                        "event_id": "evt-100",
                        "change_type": "create",
                        "data": {"event_type": "note", "summary": "Note 1"},
                    },
                    {
                        "event_id": "evt-101",
                        "change_type": "create",
                        "data": {"event_type": "note", "summary": "Note 2"},
                    },
                ],
            })

            ack = ws.receive_json()
            assert ack["type"] == "sync_ack"
            assert ack["applied"] == 2

    def test_second_client_sees_changes(self, client: TestClient):
        """A second client connecting after changes should see them."""
        # First client sends a change
        with client.websocket_connect(f"/ws/sync?key={API_KEY}") as ws1:
            ws1.send_json({
                "type": "sync_start",
                "device_id": "device-A",
                "device_name": "Device A",
                "platform": "test",
                "vector": {},
            })
            ws1.receive_json()  # sync_changes (empty)

            ws1.send_json({
                "type": "change",
                "event_id": "evt-200",
                "change_type": "create",
                "data": {"event_type": "task", "summary": "Synced task"},
            })
            ws1.receive_json()  # change_ack

        # Second client connects with empty vector — should see the change
        with client.websocket_connect(f"/ws/sync?key={API_KEY}") as ws2:
            ws2.send_json({
                "type": "sync_start",
                "device_id": "device-B",
                "device_name": "Device B",
                "platform": "test",
                "vector": {},
            })

            resp = ws2.receive_json()
            assert resp["type"] == "sync_changes"
            assert len(resp["changes"]) == 1
            assert resp["changes"][0]["event_id"] == "evt-200"

    def test_vector_filters_changes(self, client: TestClient):
        """Client with up-to-date vector should not receive old changes."""
        # First: create a change from device-X
        with client.websocket_connect(f"/ws/sync?key={API_KEY}") as ws:
            ws.send_json({
                "type": "sync_start",
                "device_id": "device-X",
                "device_name": "X",
                "platform": "test",
                "vector": {},
            })
            ws.receive_json()

            ws.send_json({
                "type": "change",
                "event_id": "evt-300",
                "change_type": "create",
                "data": {"event_type": "task", "summary": "Old task"},
            })
            ack = ws.receive_json()
            seq = ack["device_seq"]

        # Second client connects with vector that already includes device-X's seq
        with client.websocket_connect(f"/ws/sync?key={API_KEY}") as ws2:
            ws2.send_json({
                "type": "sync_start",
                "device_id": "device-Y",
                "device_name": "Y",
                "platform": "test",
                "vector": {"device-X": seq},
            })

            resp = ws2.receive_json()
            assert resp["type"] == "sync_changes"
            assert len(resp["changes"]) == 0
