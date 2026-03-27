"""Tests for SyncManager — multi-device sync layer."""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

import pytest

from core.ark import Ark
from core.sync import SyncManager


@pytest.fixture
def db_path(tmp_path: Path) -> Path:
    """Create a fresh Ark database and return its path."""
    path = tmp_path / "test.db"
    Ark(str(path), create=True)
    return path


@pytest.fixture
def sync(db_path: Path) -> SyncManager:
    """Create a SyncManager for device A."""
    return SyncManager(str(db_path), "device-a", "MacBook", "macos")


@pytest.fixture
def sync_b(db_path: Path) -> SyncManager:
    """Create a SyncManager for device B (same DB, simulating merge)."""
    return SyncManager(str(db_path), "device-b", "Pixel 8", "android")


# ============================================================================
# Device Registration
# ============================================================================


class TestDeviceRegistration:
    def test_register_device(self, sync: SyncManager) -> None:
        """Device should be registered in sync_devices on init."""
        with sync.connection() as conn:
            row = conn.execute(
                "SELECT * FROM sync_devices WHERE device_id = ?",
                (sync.device_id,),
            ).fetchone()
            assert row is not None
            assert row["name"] == "MacBook"
            assert row["platform"] == "macos"
            assert row["last_seen_at"] is not None

    def test_register_updates_last_seen(self, db_path: Path) -> None:
        """Re-creating SyncManager should update last_seen_at."""
        s1 = SyncManager(str(db_path), "dev-x", "Test", "linux")
        with s1.connection() as conn:
            row1 = conn.execute(
                "SELECT last_seen_at FROM sync_devices WHERE device_id = 'dev-x'"
            ).fetchone()

        s2 = SyncManager(str(db_path), "dev-x", "Test Updated", "linux")
        with s2.connection() as conn:
            row2 = conn.execute(
                "SELECT name, last_seen_at FROM sync_devices WHERE device_id = 'dev-x'"
            ).fetchone()

        assert row2["name"] == "Test Updated"
        # last_seen_at should be >= the first one
        assert row2["last_seen_at"] >= row1["last_seen_at"]


# ============================================================================
# Outbox Operations
# ============================================================================


class TestOutbox:
    def test_record_change(self, sync: SyncManager) -> None:
        """record_change should add entry to outbox and return seq."""
        seq = sync.record_change("evt-1", "create", {"event_type": "note", "data": {}})
        assert seq == 1

        seq2 = sync.record_change("evt-2", "update", {"event_type": "note", "data": {"text": "hi"}})
        assert seq2 == 2

    def test_record_change_invalid_type(self, sync: SyncManager) -> None:
        """Invalid change_type should raise ValueError."""
        with pytest.raises(ValueError, match="Invalid change_type"):
            sync.record_change("evt-1", "invalid", {})

    def test_get_outbox(self, sync: SyncManager) -> None:
        """get_outbox should return all unsynced entries."""
        sync.record_change("evt-1", "create", {"event_type": "note"})
        sync.record_change("evt-2", "update", {"event_type": "task"})

        outbox = sync.get_outbox()
        assert len(outbox) == 2
        assert outbox[0].event_id == "evt-1"
        assert outbox[0].change_type == "create"
        assert outbox[0].device_seq == 1
        assert outbox[1].event_id == "evt-2"
        assert outbox[1].device_seq == 2

    def test_get_outbox_empty(self, sync: SyncManager) -> None:
        """Empty outbox should return empty list."""
        assert sync.get_outbox() == []

    def test_clear_outbox(self, sync: SyncManager) -> None:
        """clear_outbox should mark entries as synced."""
        sync.record_change("evt-1", "create", {"a": 1})
        sync.record_change("evt-2", "create", {"b": 2})
        sync.record_change("evt-3", "create", {"c": 3})

        cleared = sync.clear_outbox(up_to_seq=2)
        assert cleared == 2

        remaining = sync.get_outbox()
        assert len(remaining) == 1
        assert remaining[0].event_id == "evt-3"

    def test_clear_outbox_idempotent(self, sync: SyncManager) -> None:
        """Clearing already-cleared entries should return 0."""
        sync.record_change("evt-1", "create", {})
        sync.clear_outbox(up_to_seq=1)
        cleared_again = sync.clear_outbox(up_to_seq=1)
        assert cleared_again == 0


# ============================================================================
# Version Vector
# ============================================================================


class TestVersionVector:
    def test_get_vector_empty(self, db_path: Path) -> None:
        """Fresh device with no changes should have empty or self-only vector."""
        s = SyncManager(str(db_path), "fresh-dev", "Fresh", "linux")
        vector = s.get_vector()
        # No changes recorded yet, vector may be empty
        assert isinstance(vector, dict)

    def test_vector_updates_on_record(self, sync: SyncManager) -> None:
        """Recording a change should update own vector."""
        sync.record_change("evt-1", "create", {})
        vector = sync.get_vector()
        assert vector[sync.device_id] == 1

        sync.record_change("evt-2", "create", {})
        vector = sync.get_vector()
        assert vector[sync.device_id] == 2

    def test_update_vector_for_peer(self, sync: SyncManager) -> None:
        """update_vector should track what we've seen from a peer."""
        sync.update_vector("device-b", 5)
        vector = sync.get_vector()
        assert vector["device-b"] == 5

        # Update to higher value
        sync.update_vector("device-b", 10)
        vector = sync.get_vector()
        assert vector["device-b"] == 10

    def test_update_vector_no_downgrade(self, sync: SyncManager) -> None:
        """update_vector should never decrease (MAX semantics)."""
        sync.update_vector("device-b", 10)
        sync.update_vector("device-b", 5)  # older value
        vector = sync.get_vector()
        assert vector["device-b"] == 10


# ============================================================================
# get_changes_since
# ============================================================================


class TestGetChangesSince:
    def test_returns_all_when_empty_vector(self, sync: SyncManager) -> None:
        """With empty vector, peer should get all changes."""
        sync.record_change("evt-1", "create", {"a": 1})
        sync.record_change("evt-2", "create", {"b": 2})

        changes = sync.get_changes_since({})
        assert len(changes) == 2

    def test_returns_only_unseen(self, sync: SyncManager) -> None:
        """Peer with partial vector should only get unseen changes."""
        sync.record_change("evt-1", "create", {"a": 1})
        sync.record_change("evt-2", "create", {"b": 2})
        sync.record_change("evt-3", "create", {"c": 3})

        # Peer has seen up to seq 1
        changes = sync.get_changes_since({sync.device_id: 1})
        assert len(changes) == 2
        assert changes[0].device_seq == 2
        assert changes[1].device_seq == 3

    def test_returns_empty_when_up_to_date(self, sync: SyncManager) -> None:
        """Peer that has seen everything should get empty list."""
        sync.record_change("evt-1", "create", {})
        sync.record_change("evt-2", "create", {})

        changes = sync.get_changes_since({sync.device_id: 2})
        assert len(changes) == 0

    def test_multi_device_changes(self, sync: SyncManager, sync_b: SyncManager) -> None:
        """Should return changes from multiple devices."""
        sync.record_change("evt-1", "create", {"from": "a"})
        sync_b.record_change("evt-2", "create", {"from": "b"})

        # Peer has seen nothing
        changes = sync.get_changes_since({})
        device_ids = {c.device_id for c in changes}
        assert "device-a" in device_ids
        assert "device-b" in device_ids


# ============================================================================
# Apply Remote Changes
# ============================================================================


class TestApplyRemoteChanges:
    def test_apply_no_conflict(self, sync: SyncManager) -> None:
        """Applying remote create with no local conflict should succeed."""
        result = sync.apply_remote_changes([
            {
                "event_id": "remote-evt-1",
                "change_type": "create",
                "data": {"event_type": "note", "updated_at": "2026-01-01T00:00:00Z"},
                "device_id": "device-b",
                "device_seq": 1,
            }
        ])
        assert result["applied"] == 1
        assert result["conflicts"] == 0
        assert result["skipped"] == 0

        # Vector should be updated
        vector = sync.get_vector()
        assert vector["device-b"] == 1

    def test_apply_skips_duplicate(self, sync: SyncManager) -> None:
        """Applying the same change twice should skip the second time."""
        change = {
            "event_id": "remote-evt-1",
            "change_type": "create",
            "data": {"event_type": "note"},
            "device_id": "device-b",
            "device_seq": 1,
        }
        sync.apply_remote_changes([change])
        result = sync.apply_remote_changes([change])
        assert result["skipped"] == 1
        assert result["applied"] == 0

    def test_apply_detects_conflict(self, sync: SyncManager) -> None:
        """Remote update on locally modified event should create conflict."""
        # Local change
        sync.record_change("evt-shared", "update", {
            "event_type": "task",
            "summary": "local version",
            "updated_at": "2026-01-01T12:00:00Z",
        })

        # Remote change for same event
        result = sync.apply_remote_changes([
            {
                "event_id": "evt-shared",
                "change_type": "update",
                "data": {
                    "event_type": "task",
                    "summary": "remote version",
                    "updated_at": "2026-01-01T12:01:00Z",
                },
                "device_id": "device-b",
                "device_seq": 1,
            }
        ])
        assert result["conflicts"] == 1
        assert result["applied"] == 0

        # Conflict should be stored
        conflicts = sync.get_conflicts()
        assert len(conflicts) == 1
        assert conflicts[0].event_id == "evt-shared"


# ============================================================================
# Conflict Detection & Resolution
# ============================================================================


class TestConflicts:
    def test_detect_conflict(self, sync: SyncManager) -> None:
        """detect_conflict should store conflict when data differs."""
        local = {"summary": "Buy milk", "updated_at": "2026-01-01T10:00:00Z"}
        remote = {"summary": "Buy kefir", "updated_at": "2026-01-01T10:05:00Z"}

        conflict_id = sync.detect_conflict("evt-1", local, remote, "device-b")
        assert conflict_id is not None

        conflicts = sync.get_conflicts()
        assert len(conflicts) == 1
        assert conflicts[0].id == conflict_id
        assert conflicts[0].local_data == local
        assert conflicts[0].remote_data == remote
        assert conflicts[0].local_device == "device-a"
        assert conflicts[0].remote_device == "device-b"

    def test_detect_no_conflict_when_same(self, sync: SyncManager) -> None:
        """detect_conflict should return None when data is identical."""
        data = {"summary": "Same", "updated_at": "2026-01-01T10:00:00Z"}
        result = sync.detect_conflict("evt-1", data, data, "device-b")
        assert result is None

    def test_get_conflicts_excludes_resolved(self, sync: SyncManager) -> None:
        """get_conflicts() should only return unresolved by default."""
        sync.detect_conflict("evt-1", {"a": 1}, {"a": 2}, "device-b")
        cid = sync.detect_conflict("evt-2", {"b": 1}, {"b": 2}, "device-b")
        assert cid is not None

        sync.resolve_conflict(cid, "local")

        unresolved = sync.get_conflicts()
        assert len(unresolved) == 1
        assert unresolved[0].event_id == "evt-1"

        all_conflicts = sync.get_conflicts(include_resolved=True)
        assert len(all_conflicts) == 2

    def test_resolve_conflict_local(self, sync: SyncManager) -> None:
        """Resolving with 'local' should mark as resolved."""
        cid = sync.detect_conflict("evt-1", {"v": 1}, {"v": 2}, "device-b")
        assert cid is not None

        result = sync.resolve_conflict(cid, "local")
        assert result is True

        conflicts = sync.get_conflicts(include_resolved=True)
        resolved = [c for c in conflicts if c.resolved]
        assert len(resolved) == 1
        assert resolved[0].resolution == "local"

    def test_resolve_conflict_remote(self, sync: SyncManager) -> None:
        """Resolving with 'remote' should mark as resolved."""
        cid = sync.detect_conflict("evt-1", {"v": 1}, {"v": 2}, "device-b")
        assert cid is not None
        assert sync.resolve_conflict(cid, "remote") is True

    def test_resolve_conflict_manual(self, sync: SyncManager) -> None:
        """Resolving with 'manual' should mark as resolved."""
        cid = sync.detect_conflict("evt-1", {"v": 1}, {"v": 2}, "device-b")
        assert cid is not None
        assert sync.resolve_conflict(cid, "manual") is True

    def test_resolve_invalid_resolution(self, sync: SyncManager) -> None:
        """Invalid resolution value should raise ValueError."""
        cid = sync.detect_conflict("evt-1", {"v": 1}, {"v": 2}, "device-b")
        assert cid is not None
        with pytest.raises(ValueError, match="Invalid resolution"):
            sync.resolve_conflict(cid, "invalid")

    def test_resolve_nonexistent_conflict(self, sync: SyncManager) -> None:
        """Resolving a non-existent conflict should return False."""
        result = sync.resolve_conflict("nonexistent-id", "local")
        assert result is False

    def test_resolve_already_resolved(self, sync: SyncManager) -> None:
        """Resolving an already-resolved conflict should return False."""
        cid = sync.detect_conflict("evt-1", {"v": 1}, {"v": 2}, "device-b")
        assert cid is not None
        sync.resolve_conflict(cid, "local")
        result = sync.resolve_conflict(cid, "remote")
        assert result is False
