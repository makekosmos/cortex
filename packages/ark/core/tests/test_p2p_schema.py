"""Tests for P2P sync schema: sync_peers, sync_mesh, and new columns."""

import json
import tempfile
from pathlib import Path

import pytest

from core.ark import Ark


@pytest.fixture
def db(tmp_path):
    """Create a fresh Ark database for each test."""
    db_path = tmp_path / "test_p2p.db"
    return Ark(db_path)


# ========================================================================
# sync_peers CRUD
# ========================================================================


class TestSyncPeersCrud:
    def test_insert_and_get_peer(self, db):
        db.upsert_mesh("mesh-1", "hash123")
        db.upsert_peer("peer-1", "MacBook", "macos", "mesh-1", "lan", "192.168.1.10")

        peer = db.get_peer("peer-1")
        assert peer is not None
        assert peer["peer_id"] == "peer-1"
        assert peer["name"] == "MacBook"
        assert peer["platform"] == "macos"
        assert peer["mesh_id"] == "mesh-1"
        assert peer["connection_type"] == "lan"
        assert peer["address"] == "192.168.1.10"
        assert peer["created_at"] is not None

    def test_get_nonexistent_peer(self, db):
        assert db.get_peer("no-such-peer") is None

    def test_update_peer_via_upsert(self, db):
        db.upsert_mesh("mesh-1", "hash123")
        db.upsert_peer("peer-1", "MacBook", "macos", "mesh-1")
        db.upsert_peer("peer-1", "MacBook Pro", "macos", "mesh-1", "relay", "relay.example.com")

        peer = db.get_peer("peer-1")
        assert peer["name"] == "MacBook Pro"
        assert peer["connection_type"] == "relay"
        assert peer["address"] == "relay.example.com"

    def test_update_peer_connection(self, db):
        db.upsert_mesh("mesh-1", "hash123")
        db.upsert_peer("peer-1", "Pixel", "android", "mesh-1")

        assert db.update_peer_connection("peer-1", "lan", "192.168.1.20")
        peer = db.get_peer("peer-1")
        assert peer["last_connected"] is not None
        assert peer["connection_type"] == "lan"
        assert peer["address"] == "192.168.1.20"

    def test_update_peer_connection_nonexistent(self, db):
        assert not db.update_peer_connection("no-such-peer")

    def test_delete_peer(self, db):
        db.upsert_mesh("mesh-1", "hash123")
        db.upsert_peer("peer-1", "MacBook", "macos", "mesh-1")
        assert db.delete_peer("peer-1")
        assert db.get_peer("peer-1") is None

    def test_delete_nonexistent_peer(self, db):
        assert not db.delete_peer("no-such-peer")

    def test_list_peers(self, db):
        db.upsert_mesh("mesh-1", "hash123")
        db.upsert_mesh("mesh-2", "hash456")
        db.upsert_peer("peer-1", "MacBook", "macos", "mesh-1")
        db.upsert_peer("peer-2", "Pixel", "android", "mesh-1")
        db.upsert_peer("peer-3", "Windows PC", "windows", "mesh-2")

        all_peers = db.list_peers()
        assert len(all_peers) == 3

        mesh1_peers = db.list_peers(mesh_id="mesh-1")
        assert len(mesh1_peers) == 2
        assert all(p["mesh_id"] == "mesh-1" for p in mesh1_peers)


# ========================================================================
# sync_mesh CRUD
# ========================================================================


class TestSyncMeshCrud:
    def test_insert_and_get_mesh(self, db):
        db.upsert_mesh("mesh-1", "hash123", "wss://relay.example.com")

        mesh = db.get_mesh("mesh-1")
        assert mesh is not None
        assert mesh["mesh_id"] == "mesh-1"
        assert mesh["mesh_secret_hash"] == "hash123"
        assert mesh["relay_url"] == "wss://relay.example.com"
        assert mesh["created_at"] is not None

    def test_get_nonexistent_mesh(self, db):
        assert db.get_mesh("no-such-mesh") is None

    def test_update_mesh_via_upsert(self, db):
        db.upsert_mesh("mesh-1", "hash123")
        db.upsert_mesh("mesh-1", "hash456", "wss://new-relay.example.com")

        mesh = db.get_mesh("mesh-1")
        assert mesh["mesh_secret_hash"] == "hash456"
        assert mesh["relay_url"] == "wss://new-relay.example.com"

    def test_list_meshes(self, db):
        db.upsert_mesh("mesh-1", "hash123")
        db.upsert_mesh("mesh-2", "hash456", "wss://relay.example.com")

        meshes = db.list_meshes()
        assert len(meshes) == 2

    def test_delete_mesh_cascades_peers(self, db):
        db.upsert_mesh("mesh-1", "hash123")
        db.upsert_peer("peer-1", "MacBook", "macos", "mesh-1")
        db.upsert_peer("peer-2", "Pixel", "android", "mesh-1")

        assert db.delete_mesh("mesh-1")
        assert db.get_mesh("mesh-1") is None
        assert db.list_peers(mesh_id="mesh-1") == []

    def test_delete_nonexistent_mesh(self, db):
        assert not db.delete_mesh("no-such-mesh")


# ========================================================================
# HLC column on events
# ========================================================================


class TestHlcColumn:
    def test_hlc_column_exists_in_events(self, db):
        """Verify events table has hlc column."""
        with db.connection() as conn:
            columns = {
                row["name"]
                for row in conn.execute("PRAGMA table_info(events)").fetchall()
            }
            assert "hlc" in columns

    def test_hlc_column_is_nullable(self, db):
        """HLC should be nullable (not all events need it)."""
        event_id = db.record_event("test", {"key": "value"})
        event = db.get_event(event_id)
        assert event is not None
        # hlc is not in the Event dataclass, so check via raw query
        with db.connection() as conn:
            row = conn.execute(
                "SELECT hlc FROM events WHERE id = ?", (event_id,)
            ).fetchone()
            assert row["hlc"] is None


# ========================================================================
# Outbox P2P columns
# ========================================================================


class TestOutboxP2pColumns:
    def test_outbox_has_p2p_columns(self, db):
        """Verify sync_outbox has origin_device, origin_seq, hlc, hop_path."""
        with db.connection() as conn:
            columns = {
                row["name"]
                for row in conn.execute("PRAGMA table_info(sync_outbox)").fetchall()
            }
            assert "origin_device" in columns
            assert "origin_seq" in columns
            assert "hlc" in columns
            assert "hop_path" in columns

    def test_outbox_p2p_columns_writable(self, db):
        """Verify we can write and read the new outbox columns."""
        hop_path = json.dumps(["device-a", "device-b"])
        with db.connection() as conn:
            conn.execute(
                """
                INSERT INTO sync_outbox
                    (event_id, change_type, data, device_id, device_seq,
                     origin_device, origin_seq, hlc, hop_path)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    "evt-1", "create", '{"key": "val"}', "dev-1", 1,
                    "dev-origin", 42, "2026-03-28T00:00:00.000Z-0001-dev-origin", hop_path,
                ),
            )
            row = conn.execute(
                "SELECT * FROM sync_outbox WHERE event_id = ?", ("evt-1",)
            ).fetchone()
            assert row["origin_device"] == "dev-origin"
            assert row["origin_seq"] == 42
            assert row["hlc"] == "2026-03-28T00:00:00.000Z-0001-dev-origin"
            assert json.loads(row["hop_path"]) == ["device-a", "device-b"]
