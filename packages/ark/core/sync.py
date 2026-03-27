"""
Ark Sync Layer: Multi-device synchronization via version vectors.

Handles offline queue, conflict detection, and version vector tracking
for syncing Ark databases across devices.

Usage:
    from core.sync import SyncManager

    sync = SyncManager("life.db", "mac-abc", "MacBook Pro", "macos")
    sync.record_change(event_id, "create", {...})
    changes = sync.get_outbox()
"""

from __future__ import annotations

import json
import sqlite3
import uuid
from contextlib import contextmanager
from collections.abc import Iterator
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional, Union


# ============================================================================
# Data Classes
# ============================================================================


@dataclass
class OutboxEntry:
    """A pending change in the sync outbox."""

    id: int
    event_id: str
    change_type: str
    data: dict[str, Any]
    device_id: str
    device_seq: int
    created_at: str

    @classmethod
    def from_row(cls, row: sqlite3.Row) -> "OutboxEntry":
        return cls(
            id=row["id"],
            event_id=row["event_id"],
            change_type=row["change_type"],
            data=json.loads(row["data"]) if isinstance(row["data"], str) else row["data"],
            device_id=row["device_id"],
            device_seq=row["device_seq"],
            created_at=row["created_at"],
        )


@dataclass
class Conflict:
    """An unresolved sync conflict."""

    id: str
    event_id: str
    local_data: dict[str, Any]
    remote_data: dict[str, Any]
    local_device: str
    remote_device: str
    local_updated_at: str
    remote_updated_at: str
    resolved: bool
    resolution: Optional[str]
    created_at: str

    @classmethod
    def from_row(cls, row: sqlite3.Row) -> "Conflict":
        return cls(
            id=row["id"],
            event_id=row["event_id"],
            local_data=json.loads(row["local_data"]) if isinstance(row["local_data"], str) else row["local_data"],
            remote_data=json.loads(row["remote_data"]) if isinstance(row["remote_data"], str) else row["remote_data"],
            local_device=row["local_device"],
            remote_device=row["remote_device"],
            local_updated_at=row["local_updated_at"],
            remote_updated_at=row["remote_updated_at"],
            resolved=bool(row["resolved"]),
            resolution=row["resolution"],
            created_at=row["created_at"],
        )


# ============================================================================
# Utility Functions
# ============================================================================


def _generate_uuid() -> str:
    return str(uuid.uuid4())


def _utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")


# ============================================================================
# SyncManager
# ============================================================================


class SyncManager:
    """
    Manages multi-device synchronization for an Ark database.

    Each device has a unique device_id and maintains:
    - A monotonic sequence counter (device_seq)
    - A version vector tracking what it has seen from peers
    - An outbox of unsynced local changes

    Example:
        sync = SyncManager("life.db", "mac-abc", "MacBook Pro", "macos")
        sync.record_change(event_id, "create", {"event_type": "note", ...})
        outbox = sync.get_outbox()
    """

    def __init__(
        self,
        db_path: Union[str, Path],
        device_id: str,
        device_name: str,
        platform: str,
    ):
        self.db_path = Path(db_path)
        self.device_id = device_id
        self.device_name = device_name
        self.platform = platform

        self._ensure_sync_tables()
        self._register_device()
        self._ensure_sync_columns()

    @contextmanager
    def connection(self) -> Iterator[sqlite3.Connection]:
        """Context manager for database connection, matching Ark patterns."""
        conn = sqlite3.connect(
            self.db_path,
            detect_types=sqlite3.PARSE_DECLTYPES | sqlite3.PARSE_COLNAMES,
            timeout=30,
        )
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA foreign_keys = ON")
        conn.execute("PRAGMA busy_timeout = 5000")
        try:
            yield conn
            conn.commit()
        except Exception:
            conn.rollback()
            raise
        finally:
            conn.close()

    def _ensure_sync_tables(self) -> None:
        """Create sync tables if they don't exist."""
        with self.connection() as conn:
            conn.executescript("""
                CREATE TABLE IF NOT EXISTS sync_devices (
                    device_id   TEXT PRIMARY KEY,
                    name        TEXT NOT NULL,
                    platform    TEXT NOT NULL,
                    last_seen_at TEXT,
                    created_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                );

                CREATE TABLE IF NOT EXISTS sync_vectors (
                    device_id   TEXT NOT NULL,
                    peer_id     TEXT NOT NULL,
                    last_seq    INTEGER NOT NULL,
                    PRIMARY KEY (device_id, peer_id)
                );

                CREATE TABLE IF NOT EXISTS sync_outbox (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_id    TEXT NOT NULL,
                    change_type TEXT NOT NULL,
                    data        TEXT NOT NULL,
                    device_id   TEXT NOT NULL,
                    device_seq  INTEGER NOT NULL,
                    created_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                    synced      INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS sync_conflicts (
                    id                  TEXT PRIMARY KEY,
                    event_id            TEXT NOT NULL,
                    local_data          TEXT NOT NULL,
                    remote_data         TEXT NOT NULL,
                    local_device        TEXT NOT NULL,
                    remote_device       TEXT NOT NULL,
                    local_updated_at    TEXT NOT NULL,
                    remote_updated_at   TEXT NOT NULL,
                    resolved            INTEGER DEFAULT 0,
                    resolution          TEXT,
                    created_at          TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                );
            """)

    def _ensure_sync_columns(self) -> None:
        """Add device_id and device_seq columns to events if missing."""
        with self.connection() as conn:
            columns = {
                row["name"]
                for row in conn.execute("PRAGMA table_info(events)").fetchall()
            }
            if "device_id" not in columns:
                conn.execute("ALTER TABLE events ADD COLUMN device_id TEXT")
            if "device_seq" not in columns:
                conn.execute("ALTER TABLE events ADD COLUMN device_seq INTEGER")

    def _register_device(self) -> None:
        """Register this device in sync_devices."""
        with self.connection() as conn:
            conn.execute(
                """
                INSERT INTO sync_devices (device_id, name, platform, last_seen_at)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(device_id) DO UPDATE SET
                    name = excluded.name,
                    platform = excluded.platform,
                    last_seen_at = excluded.last_seen_at
                """,
                (self.device_id, self.device_name, self.platform, _utc_now()),
            )

    def _next_seq(self, conn: sqlite3.Connection) -> int:
        """Get the next monotonic sequence number for this device."""
        row = conn.execute(
            """
            SELECT COALESCE(MAX(device_seq), 0) + 1 AS next_seq
            FROM sync_outbox
            WHERE device_id = ?
            """,
            (self.device_id,),
        ).fetchone()
        return int(row["next_seq"])

    # ========================================================================
    # Outbox Operations
    # ========================================================================

    def record_change(
        self,
        event_id: str,
        change_type: str,
        data: dict[str, Any],
    ) -> int:
        """
        Record a local change in the outbox for later sync.

        Args:
            event_id: ID of the event that changed
            change_type: One of 'create', 'update', 'delete'
            data: Full event data as dict

        Returns:
            The device_seq assigned to this change
        """
        if change_type not in ("create", "update", "delete"):
            raise ValueError(f"Invalid change_type: {change_type}")

        with self.connection() as conn:
            seq = self._next_seq(conn)
            conn.execute(
                """
                INSERT INTO sync_outbox (event_id, change_type, data, device_id, device_seq)
                VALUES (?, ?, ?, ?, ?)
                """,
                (
                    event_id,
                    change_type,
                    json.dumps(data, ensure_ascii=False),
                    self.device_id,
                    seq,
                ),
            )
            # Update our own vector to reflect our latest seq
            conn.execute(
                """
                INSERT INTO sync_vectors (device_id, peer_id, last_seq)
                VALUES (?, ?, ?)
                ON CONFLICT(device_id, peer_id) DO UPDATE SET last_seq = excluded.last_seq
                """,
                (self.device_id, self.device_id, seq),
            )
            return seq

    def get_outbox(self) -> list[OutboxEntry]:
        """Return all unsynced changes from the outbox."""
        with self.connection() as conn:
            rows = conn.execute(
                """
                SELECT * FROM sync_outbox
                WHERE synced = 0 AND device_id = ?
                ORDER BY device_seq ASC
                """,
                (self.device_id,),
            ).fetchall()
            return [OutboxEntry.from_row(row) for row in rows]

    def clear_outbox(self, up_to_seq: int) -> int:
        """
        Mark outbox entries as synced up to and including the given seq.

        Returns:
            Number of entries marked as synced
        """
        with self.connection() as conn:
            cur = conn.execute(
                """
                UPDATE sync_outbox
                SET synced = 1
                WHERE device_id = ? AND device_seq <= ? AND synced = 0
                """,
                (self.device_id, up_to_seq),
            )
            return cur.rowcount

    # ========================================================================
    # Version Vector Operations
    # ========================================================================

    def get_vector(self) -> dict[str, int]:
        """
        Return this device's version vector.

        Returns:
            Dict mapping device_id -> last_seen_seq
        """
        with self.connection() as conn:
            rows = conn.execute(
                "SELECT peer_id, last_seq FROM sync_vectors WHERE device_id = ?",
                (self.device_id,),
            ).fetchall()
            return {row["peer_id"]: row["last_seq"] for row in rows}

    def update_vector(self, peer_id: str, last_seq: int) -> None:
        """
        Update what we have seen from a peer.

        Args:
            peer_id: The peer device whose seq we are acknowledging
            last_seq: The latest seq we have processed from this peer
        """
        with self.connection() as conn:
            conn.execute(
                """
                INSERT INTO sync_vectors (device_id, peer_id, last_seq)
                VALUES (?, ?, ?)
                ON CONFLICT(device_id, peer_id) DO UPDATE SET
                    last_seq = MAX(excluded.last_seq, sync_vectors.last_seq)
                """,
                (self.device_id, peer_id, last_seq),
            )

    # ========================================================================
    # Sync Protocol
    # ========================================================================

    def get_changes_since(self, vector: dict[str, int]) -> list[OutboxEntry]:
        """
        Return all changes that a peer hasn't seen, based on their vector.

        Args:
            vector: The peer's version vector {device_id: last_seen_seq}

        Returns:
            List of outbox entries the peer is missing
        """
        results: list[OutboxEntry] = []
        with self.connection() as conn:
            # Get all devices that have entries in the outbox
            device_rows = conn.execute(
                "SELECT DISTINCT device_id FROM sync_outbox WHERE synced = 0 OR 1=1"
            ).fetchall()

            for device_row in device_rows:
                did = device_row["device_id"]
                last_seen = vector.get(did, 0)
                rows = conn.execute(
                    """
                    SELECT * FROM sync_outbox
                    WHERE device_id = ? AND device_seq > ?
                    ORDER BY device_seq ASC
                    """,
                    (did, last_seen),
                ).fetchall()
                results.extend(OutboxEntry.from_row(row) for row in rows)

        return results

    def apply_remote_changes(self, changes: list[dict[str, Any]]) -> dict[str, int]:
        """
        Apply changes received from a remote peer.

        Each change dict should have:
            event_id, change_type, data, device_id, device_seq

        For each change:
        - If no local version exists, apply directly
        - If local version exists and was modified by a different device,
          detect conflict

        Returns:
            Dict with counts: {"applied": N, "conflicts": N, "skipped": N}
        """
        applied = 0
        conflicts = 0
        skipped = 0

        with self.connection() as conn:
            for change in changes:
                event_id = change["event_id"]
                change_type = change["change_type"]
                remote_data = change["data"] if isinstance(change["data"], dict) else json.loads(change["data"])
                remote_device = change["device_id"]
                remote_seq = change["device_seq"]

                # Check if we already have this exact change (idempotency)
                existing = conn.execute(
                    """
                    SELECT 1 FROM sync_outbox
                    WHERE device_id = ? AND device_seq = ?
                    """,
                    (remote_device, remote_seq),
                ).fetchone()
                if existing:
                    skipped += 1
                    # Still update vector
                    self._update_vector_in_conn(conn, remote_device, remote_seq)
                    continue

                # Check for conflict: does a local unsent change exist for the same event?
                local_change = conn.execute(
                    """
                    SELECT * FROM sync_outbox
                    WHERE event_id = ? AND device_id = ? AND synced = 0
                    ORDER BY device_seq DESC LIMIT 1
                    """,
                    (event_id, self.device_id),
                ).fetchone()

                if local_change and change_type in ("update", "delete"):
                    local_data = json.loads(local_change["data"]) if isinstance(local_change["data"], str) else local_change["data"]
                    # Conflict detected
                    conflict_id = _generate_uuid()
                    local_updated = local_data.get("updated_at", "")
                    remote_updated = remote_data.get("updated_at", "")
                    conn.execute(
                        """
                        INSERT INTO sync_conflicts
                            (id, event_id, local_data, remote_data,
                             local_device, remote_device,
                             local_updated_at, remote_updated_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            conflict_id,
                            event_id,
                            json.dumps(local_data, ensure_ascii=False),
                            json.dumps(remote_data, ensure_ascii=False),
                            self.device_id,
                            remote_device,
                            local_updated,
                            remote_updated,
                        ),
                    )
                    conflicts += 1
                else:
                    # No conflict — store in outbox as a record of what we received
                    conn.execute(
                        """
                        INSERT INTO sync_outbox
                            (event_id, change_type, data, device_id, device_seq, synced)
                        VALUES (?, ?, ?, ?, ?, 1)
                        """,
                        (
                            event_id,
                            change_type,
                            json.dumps(remote_data, ensure_ascii=False),
                            remote_device,
                            remote_seq,
                        ),
                    )
                    applied += 1

                # Update our vector for this peer
                self._update_vector_in_conn(conn, remote_device, remote_seq)

        return {"applied": applied, "conflicts": conflicts, "skipped": skipped}

    def _update_vector_in_conn(
        self, conn: sqlite3.Connection, peer_id: str, last_seq: int
    ) -> None:
        """Update version vector within an existing connection/transaction."""
        conn.execute(
            """
            INSERT INTO sync_vectors (device_id, peer_id, last_seq)
            VALUES (?, ?, ?)
            ON CONFLICT(device_id, peer_id) DO UPDATE SET
                last_seq = MAX(excluded.last_seq, sync_vectors.last_seq)
            """,
            (self.device_id, peer_id, last_seq),
        )

    # ========================================================================
    # Conflict Management
    # ========================================================================

    def detect_conflict(
        self,
        event_id: str,
        local_data: dict[str, Any],
        remote_data: dict[str, Any],
        remote_device: str = "unknown",
    ) -> Optional[str]:
        """
        Check if local and remote data conflict and store if so.

        A conflict exists when both sides modified the same event
        (different data for the same event_id).

        Args:
            event_id: The event in question
            local_data: Local version of the event
            remote_data: Remote version of the event
            remote_device: Device ID of the remote peer

        Returns:
            Conflict ID if conflict detected, None otherwise
        """
        # No conflict if data is identical
        if local_data == remote_data:
            return None

        conflict_id = _generate_uuid()
        local_updated = local_data.get("updated_at", "")
        remote_updated = remote_data.get("updated_at", "")

        with self.connection() as conn:
            conn.execute(
                """
                INSERT INTO sync_conflicts
                    (id, event_id, local_data, remote_data,
                     local_device, remote_device,
                     local_updated_at, remote_updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    conflict_id,
                    event_id,
                    json.dumps(local_data, ensure_ascii=False),
                    json.dumps(remote_data, ensure_ascii=False),
                    self.device_id,
                    remote_device,
                    local_updated,
                    remote_updated,
                ),
            )
        return conflict_id

    def get_conflicts(self, include_resolved: bool = False) -> list[Conflict]:
        """
        Return sync conflicts.

        Args:
            include_resolved: If True, return all conflicts; otherwise only unresolved
        """
        with self.connection() as conn:
            if include_resolved:
                rows = conn.execute(
                    "SELECT * FROM sync_conflicts ORDER BY created_at DESC"
                ).fetchall()
            else:
                rows = conn.execute(
                    "SELECT * FROM sync_conflicts WHERE resolved = 0 ORDER BY created_at DESC"
                ).fetchall()
            return [Conflict.from_row(row) for row in rows]

    def resolve_conflict(self, conflict_id: str, resolution: str) -> bool:
        """
        Resolve a conflict.

        Args:
            conflict_id: ID of the conflict to resolve
            resolution: One of 'local', 'remote', 'manual'

        Returns:
            True if conflict was found and resolved, False otherwise
        """
        if resolution not in ("local", "remote", "manual"):
            raise ValueError(f"Invalid resolution: {resolution}. Must be 'local', 'remote', or 'manual'")

        with self.connection() as conn:
            cur = conn.execute(
                """
                UPDATE sync_conflicts
                SET resolved = 1, resolution = ?
                WHERE id = ? AND resolved = 0
                """,
                (resolution, conflict_id),
            )
            return cur.rowcount == 1
