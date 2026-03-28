"""
Ark Core: Personal Life Database

Minimal event storage library. No predefined categories or event types.
All semantics come from plugins.

Usage:
    from core import Ark

    db = Ark("life.db")
    db.record_event("heart_rate", {"bpm": 72}, category="health")
"""

from __future__ import annotations

import json
import re
import sqlite3
import threading
import uuid
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import asdict, dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Optional, Union

# ============================================================================
# Data Classes
# ============================================================================


@dataclass
class Event:
    """Represents a single event."""

    id: str
    event_type: str
    occurred_at: str
    data: dict[str, Any]
    category: Optional[str] = None
    summary: Optional[str] = None
    duration_seconds: Optional[int] = None
    timezone: Optional[str] = None
    source: Optional[str] = None
    source_id: Optional[str] = None
    device: Optional[str] = None
    tags: list[str] = field(default_factory=list)
    created_at: Optional[str] = None
    updated_at: Optional[str] = None

    @classmethod
    def from_row(cls, row: sqlite3.Row) -> "Event":
        """Create Event from database row."""
        return cls(
            id=row["id"],
            event_type=row["event_type"],
            occurred_at=row["occurred_at"],
            data=json.loads(row["data"]) if row["data"] else {},
            category=row["category"],
            summary=row["summary"],
            duration_seconds=row["duration_seconds"],
            timezone=row["timezone"],
            source=row["source"],
            source_id=row["source_id"],
            device=row["device"],
            tags=json.loads(row["tags"]) if row["tags"] else [],
            created_at=row["created_at"],
            updated_at=row["updated_at"],
        )


@dataclass
class Entity:
    """Represents an entity (person, place, project, etc.)."""

    id: str
    entity_type: str
    name: str
    data: dict[str, Any] = field(default_factory=dict)
    aliases: list[str] = field(default_factory=list)
    is_active: bool = True
    created_at: Optional[str] = None
    updated_at: Optional[str] = None

    @classmethod
    def from_row(cls, row: sqlite3.Row) -> "Entity":
        """Create Entity from database row."""
        return cls(
            id=row["id"],
            entity_type=row["entity_type"],
            name=row["name"],
            data=json.loads(row["data"]) if row["data"] else {},
            aliases=json.loads(row["aliases"]) if row["aliases"] else [],
            is_active=bool(row["is_active"]),
            created_at=row["created_at"],
            updated_at=row["updated_at"],
        )


# ============================================================================
# Utility Functions
# ============================================================================


def generate_uuid() -> str:
    """Generate a UUID v4 string."""
    return str(uuid.uuid4())


def utc_now() -> str:
    """Get current UTC timestamp in ISO 8601 format."""
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def to_iso8601(dt: datetime) -> str:
    """Convert datetime to ISO 8601 UTC string."""
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def sanitize_fts_query(query: str) -> str:
    """Strip FTS5 special characters/operators from user input to prevent injection."""
    # Remove FTS5 operators (as whole words) and special characters
    _FTS5_OPERATORS = re.compile(r'\b(NEAR|AND|OR|NOT)\b', re.IGNORECASE)
    cleaned = _FTS5_OPERATORS.sub(' ', query)
    # Remove special FTS5 characters: * " ( ) + - ^ :
    cleaned = re.sub(r'[*"()+\-^:]', ' ', cleaned)
    # Collapse whitespace
    return re.sub(r'\s+', ' ', cleaned).strip()


def parse_iso8601(s: str) -> datetime:
    """Parse ISO 8601 string to datetime."""
    formats = [
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%dT%H:%M:%S%z",
        "%Y-%m-%dT%H:%M:%S.%fZ",
        "%Y-%m-%dT%H:%M:%S.%f%z",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S.%f",
    ]

    for fmt in formats:
        try:
            dt = datetime.strptime(s.replace("Z", "+0000"), fmt.replace("Z", "%z"))
            if dt.tzinfo:
                return dt.astimezone(timezone.utc)
            return dt.replace(tzinfo=timezone.utc)
        except ValueError:
            continue

    raise ValueError(f"Cannot parse timestamp: {s}")


# ============================================================================
# Main Ark Class
# ============================================================================


class Ark:
    """
    Core interface to the Ark database.

    Example:
        db = Ark("my_life.db")
        db.record_event("heart_rate", {"bpm": 72}, category="health")
    """

    def __init__(
        self,
        db_path: Union[str, Path],
        create: bool = True,
        keep_alive: bool = False,
    ):
        """
        Initialize Ark connection.

        Args:
            db_path: Path to SQLite database file
            create: If True, create database if it doesn't exist
            keep_alive: If True, keep a persistent connection open per thread
                        instead of opening/closing on every operation.
        """
        self.db_path = Path(db_path)
        self._keep_alive = keep_alive
        self._local = threading.local() if keep_alive else None

        if self.db_path.exists():
            if self.db_path.is_dir():
                raise IsADirectoryError(f"Database path is a directory: {self.db_path}")

            # If the file exists but has no schema (e.g., empty file), initialize it
            if create and not self._is_database_initialized():
                self._initialize_database()
        else:
            if not create:
                raise FileNotFoundError(f"Database file not found: {self.db_path}")
            self.db_path.parent.mkdir(parents=True, exist_ok=True)
            self._initialize_database()

    def _is_database_initialized(self) -> bool:
        """Return True if the database appears to contain the core schema."""
        try:
            with self.connection() as conn:
                row = conn.execute(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='events'"
                ).fetchone()
                return row is not None
        except sqlite3.Error:
            # If it's not a valid SQLite file or can't be opened, treat as uninitialized.
            return False

    def _initialize_database(self) -> None:
        """Create and initialize a new database."""
        schema_path = Path(__file__).parent / "schema.sql"
        if not schema_path.exists():
            raise FileNotFoundError(f"Schema file not found: {schema_path}")

        with self.connection() as conn, open(schema_path, encoding="utf-8") as f:
            conn.executescript(f.read())

    def _normalize_occurred_at(
        self, occurred_at: Optional[Union[str, datetime]]
    ) -> str:
        """Normalize timestamps to a single canonical format: UTC, `YYYY-MM-DDTHH:MM:SSZ`."""
        if occurred_at is None:
            return utc_now()
        if isinstance(occurred_at, datetime):
            return to_iso8601(occurred_at)
        # Accept various ISO-8601 strings and normalize them to UTC Z.
        return to_iso8601(parse_iso8601(occurred_at))

    def _make_connection(self) -> sqlite3.Connection:
        """Create a new SQLite connection with standard pragmas."""
        conn = sqlite3.connect(
            self.db_path,
            detect_types=sqlite3.PARSE_DECLTYPES | sqlite3.PARSE_COLNAMES,
            timeout=30,
        )
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA foreign_keys = ON")
        conn.execute("PRAGMA busy_timeout = 5000")
        # Prefer durability over raw write throughput. Individual apps can override.
        conn.execute("PRAGMA synchronous = FULL")
        return conn

    def _get_persistent_connection(self) -> sqlite3.Connection:
        """Return the thread-local persistent connection, creating it if needed."""
        assert self._local is not None
        conn = getattr(self._local, "conn", None)
        if conn is None:
            conn = self._make_connection()
            self._local.conn = conn
        return conn

    @contextmanager
    def connection(self) -> Iterator[sqlite3.Connection]:
        """Context manager for database connection.

        When *keep_alive* is True the connection is kept open across calls
        (one per thread via ``threading.local``).  When False, a fresh
        connection is opened and closed for every operation.
        """
        if self._keep_alive:
            conn = self._get_persistent_connection()
            try:
                yield conn
                conn.commit()
            except Exception:
                conn.rollback()
                raise
        else:
            conn = self._make_connection()
            try:
                yield conn
                conn.commit()
            except Exception:
                conn.rollback()
                raise
            finally:
                conn.close()

    # -- Lifecycle helpers ---------------------------------------------------

    def close(self) -> None:
        """Close the persistent connection (no-op when *keep_alive* is False).

        Safe to call multiple times.
        """
        if self._local is not None:
            conn = getattr(self._local, "conn", None)
            if conn is not None:
                try:
                    conn.close()
                except Exception:
                    pass
                self._local.conn = None

    def __enter__(self) -> "Ark":
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.close()

    # ========================================================================
    # Event Recording
    # ========================================================================

    def record_event(
        self,
        event_type: str,
        data: dict[str, Any],
        category: Optional[str] = None,
        occurred_at: Optional[Union[str, datetime]] = None,
        summary: Optional[str] = None,
        duration_seconds: Optional[int] = None,
        timezone: Optional[str] = None,
        source: Optional[str] = None,
        source_id: Optional[str] = None,
        device: Optional[str] = None,
        tags: Optional[list[str]] = None,
        entity_ids: Optional[list[str]] = None,
    ) -> str:
        """
        Record a new event.

        Args:
            event_type: Type of event (plugin-defined)
            data: Event payload as dictionary
            category: Category (plugin-defined, optional)
            occurred_at: When event happened (default: now)
            summary: Human-readable summary
            duration_seconds: Duration if applicable
            timezone: Original timezone
            source: Data source (e.g., 'garmin', 'manual')
            source_id: ID in source system
            device: Recording device
            tags: List of tags
            entity_ids: List of entity IDs to link

        Returns:
            ID of created event
        """
        event_id = generate_uuid()

        occurred_at_str = self._normalize_occurred_at(occurred_at)

        with self.connection() as conn:
            if source and source_id:
                # Upsert: keep stable ID for the (source, source_id) pair.
                #
                # IMPORTANT: Use RETURNING to get the *actual* row id on both insert and
                # update (race-safe when multiple writers hit the same source/source_id).
                row = conn.execute(
                    """
                    INSERT INTO events (
                        id, event_type, category, occurred_at, data, summary,
                        duration_seconds, timezone, source, source_id, device, tags
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(source, source_id) DO UPDATE SET
                        event_type = excluded.event_type,
                        category = excluded.category,
                        occurred_at = excluded.occurred_at,
                        data = excluded.data,
                        summary = excluded.summary,
                        duration_seconds = excluded.duration_seconds,
                        timezone = excluded.timezone,
                        device = excluded.device,
                        tags = excluded.tags,
                        is_deleted = 0
                    RETURNING id
                    """,
                    (
                        event_id,
                        event_type,
                        category,
                        occurred_at_str,
                        json.dumps(data, ensure_ascii=False),
                        summary,
                        duration_seconds,
                        timezone,
                        source,
                        source_id,
                        device,
                        json.dumps(tags or [], ensure_ascii=False),
                    ),
                ).fetchone()
                # SQLite guarantees a returned row for INSERT ... RETURNING.
                if row is not None:
                    event_id = str(row[0])
            else:
                conn.execute(
                    """
                    INSERT INTO events (
                        id, event_type, category, occurred_at, data, summary,
                        duration_seconds, timezone, source, source_id, device, tags
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        event_id,
                        event_type,
                        category,
                        occurred_at_str,
                        json.dumps(data, ensure_ascii=False),
                        summary,
                        duration_seconds,
                        timezone,
                        source,
                        source_id,
                        device,
                        json.dumps(tags or [], ensure_ascii=False),
                    ),
                )

            if entity_ids:
                for entity_id in entity_ids:
                    self._link_event_entity(conn, event_id, entity_id)

        return event_id

    def record_events_batch(
        self,
        events: list[dict[str, Any]],
        source: Optional[str] = None,
    ) -> tuple[int, int, int]:
        """
        Record multiple events in a single transaction.

        Args:
            events: List of event dictionaries
            source: Default source for all events

        Returns:
            Tuple of (created, updated, skipped) counts
        """
        created = 0
        updated = 0
        skipped = 0

        with self.connection() as conn:
            for event_data in events:
                event_type = event_data.get("event_type")
                if not event_type:
                    skipped += 1
                    continue

                data = event_data.get("data", {})
                event_source = event_data.get("source", source)
                source_id = event_data.get("source_id")

                occurred_at = event_data.get("occurred_at")
                occurred_at_str = self._normalize_occurred_at(occurred_at)

                if event_source and source_id:
                    before = conn.total_changes
                    conn.execute(
                        """
                        INSERT OR IGNORE INTO events (
                            id, event_type, category, occurred_at, data, summary,
                            duration_seconds, timezone, source, source_id, device, tags
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            generate_uuid(),
                            event_type,
                            event_data.get("category"),
                            occurred_at_str,
                            json.dumps(data, ensure_ascii=False),
                            event_data.get("summary"),
                            event_data.get("duration_seconds"),
                            event_data.get("timezone"),
                            event_source,
                            source_id,
                            event_data.get("device"),
                            json.dumps(event_data.get("tags", []), ensure_ascii=False),
                        ),
                    )
                    after = conn.total_changes

                    if after == before:
                        cur = conn.execute(
                            """
                            UPDATE events SET
                                event_type = ?,
                                category = ?,
                                occurred_at = ?,
                                data = ?,
                                summary = ?,
                                duration_seconds = ?,
                                timezone = ?,
                                device = ?,
                                tags = ?,
                                is_deleted = 0
                            WHERE source = ? AND source_id = ?
                            """,
                            (
                                event_type,
                                event_data.get("category"),
                                occurred_at_str,
                                json.dumps(data, ensure_ascii=False),
                                event_data.get("summary"),
                                event_data.get("duration_seconds"),
                                event_data.get("timezone"),
                                event_data.get("device"),
                                json.dumps(
                                    event_data.get("tags", []), ensure_ascii=False
                                ),
                                event_source,
                                source_id,
                            ),
                        )
                        if cur.rowcount != 1:
                            raise sqlite3.IntegrityError(
                                "Failed to upsert event: insert ignored but update matched 0 rows"
                            )
                        updated += 1
                    else:
                        created += 1
                else:
                    conn.execute(
                        """
                        INSERT INTO events (
                            id, event_type, category, occurred_at, data, summary,
                            duration_seconds, timezone, source, source_id, device, tags
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            generate_uuid(),
                            event_type,
                            event_data.get("category"),
                            occurred_at_str,
                            json.dumps(data, ensure_ascii=False),
                            event_data.get("summary"),
                            event_data.get("duration_seconds"),
                            event_data.get("timezone"),
                            event_source,
                            source_id,
                            event_data.get("device"),
                            json.dumps(event_data.get("tags", []), ensure_ascii=False),
                        ),
                    )
                    created += 1

        return created, updated, skipped

    # ========================================================================
    # Event Queries
    # ========================================================================

    def get_event(self, event_id: str) -> Optional[Event]:
        """Get event by ID."""
        with self.connection() as conn:
            row = conn.execute(
                "SELECT * FROM events WHERE id = ? AND is_deleted = 0", (event_id,)
            ).fetchone()

            return Event.from_row(row) if row else None

    def delete_event(self, event_id: str) -> bool:
        """Soft-delete event by ID (sets is_deleted=1)."""
        with self.connection() as conn:
            cur = conn.execute(
                "UPDATE events SET is_deleted = 1 WHERE id = ? AND is_deleted = 0",
                (event_id,),
            )
            return bool(cur.rowcount == 1)

    def query_events(
        self,
        event_type: Optional[str] = None,
        category: Optional[str] = None,
        start_date: Optional[Union[str, datetime]] = None,
        end_date: Optional[Union[str, datetime]] = None,
        days: Optional[int] = None,
        source: Optional[str] = None,
        tags: Optional[list[str]] = None,
        search: Optional[str] = None,
        limit: int = 1000,
        offset: int = 0,
        order: str = "DESC",
    ) -> list[Event]:
        """
        Query events with filters.

        Args:
            event_type: Filter by event type
            category: Filter by category
            start_date: Start of date range
            end_date: End of date range
            days: Last N days
            source: Filter by source
            tags: Filter by tags (any match)
            search: Full-text search query
            limit: Maximum results
            offset: Skip first N results
            order: Sort order ('ASC' or 'DESC')

        Returns:
            List of Event objects
        """
        order = order.upper()
        if order not in ("ASC", "DESC"):
            raise ValueError(f"Invalid order: {order}")

        conditions = ["e.is_deleted = 0"]
        params: list[Any] = []

        if event_type:
            conditions.append("e.event_type = ?")
            params.append(event_type)

        if category:
            conditions.append("e.category = ?")
            params.append(category)

        if days:
            start_date = datetime.now(timezone.utc) - timedelta(days=days)

        if start_date:
            if isinstance(start_date, datetime):
                start_date = to_iso8601(start_date)
            conditions.append("e.occurred_at >= ?")
            params.append(start_date)

        if end_date:
            if isinstance(end_date, datetime):
                end_date = to_iso8601(end_date)
            conditions.append("e.occurred_at <= ?")
            params.append(end_date)

        if source:
            conditions.append("e.source = ?")
            params.append(source)

        if tags:
            tag_conditions = []
            for tag in tags:
                tag_conditions.append("e.tags LIKE ?")
                params.append(f'%"{tag}"%')
            conditions.append(f"({' OR '.join(tag_conditions)})")

        if search:
            search = sanitize_fts_query(search)
            if not search:
                return []
            query = f"""
                SELECT e.* FROM events e
                JOIN events_fts fts ON e.rowid = fts.rowid
                WHERE {" AND ".join(conditions)}
                AND events_fts MATCH ?
                ORDER BY e.occurred_at {order}
                LIMIT ? OFFSET ?
            """  # noqa: S608
            params.append(search)
        else:
            query = f"""
                SELECT e.* FROM events e
                WHERE {" AND ".join(conditions)}
                ORDER BY e.occurred_at {order}
                LIMIT ? OFFSET ?
            """  # noqa: S608

        params.extend([limit, offset])

        with self.connection() as conn:
            rows = conn.execute(query, params).fetchall()
            return [Event.from_row(row) for row in rows]

    def count_events(
        self,
        event_type: Optional[str] = None,
        category: Optional[str] = None,
        start_date: Optional[Union[str, datetime]] = None,
        end_date: Optional[Union[str, datetime]] = None,
        days: Optional[int] = None,
    ) -> int:
        """Count events matching filters."""
        conditions = ["is_deleted = 0"]
        params: list[Any] = []

        if event_type:
            conditions.append("event_type = ?")
            params.append(event_type)

        if category:
            conditions.append("category = ?")
            params.append(category)

        if days:
            start_date = datetime.now(timezone.utc) - timedelta(days=days)

        if start_date:
            if isinstance(start_date, datetime):
                start_date = to_iso8601(start_date)
            conditions.append("occurred_at >= ?")
            params.append(start_date)

        if end_date:
            if isinstance(end_date, datetime):
                end_date = to_iso8601(end_date)
            conditions.append("occurred_at <= ?")
            params.append(end_date)

        query = f"SELECT COUNT(*) FROM events WHERE {' AND '.join(conditions)}"  # noqa: S608

        with self.connection() as conn:
            result = conn.execute(query, params).fetchone()
            return int(result[0]) if result else 0

    def search(self, query: str, limit: int = 100) -> list[Event]:
        """Full-text search across all events."""
        return self.query_events(search=query, limit=limit)

    def get_daily_stats(self, date: str) -> dict[str, Any]:
        """
        Get aggregated statistics for a specific date.

        Args:
            date: Date in YYYY-MM-DD format

        Returns:
            Dictionary with daily statistics
        """
        with self.connection() as conn:
            rows = conn.execute(
                """
                SELECT event_type, COUNT(*) as count
                FROM events
                WHERE substr(occurred_at, 1, 10) = ?
                AND is_deleted = 0
                GROUP BY event_type
                """,
                (date,),
            ).fetchall()

            by_type: dict[str, int] = {}
            total_events = 0

            for row in rows:
                total_events += row["count"]
                by_type[row["event_type"]] = row["count"]

            return {
                "date": date,
                "total_events": total_events,
                "by_type": by_type,
            }

    # ========================================================================
    # Entity Management
    # ========================================================================

    def create_entity(
        self,
        entity_type: str,
        name: str,
        data: Optional[dict[str, Any]] = None,
        aliases: Optional[list[str]] = None,
    ) -> str:
        """Create a new entity."""
        entity_id = generate_uuid()

        with self.connection() as conn:
            conn.execute(
                """
                INSERT INTO entities (id, entity_type, name, data, aliases)
                VALUES (?, ?, ?, ?, ?)
                """,
                (
                    entity_id,
                    entity_type,
                    name,
                    json.dumps(data or {}, ensure_ascii=False),
                    json.dumps(aliases or [], ensure_ascii=False),
                ),
            )

        return entity_id

    def get_entity(self, entity_id: str) -> Optional[Entity]:
        """Get entity by ID."""
        with self.connection() as conn:
            row = conn.execute(
                "SELECT * FROM entities WHERE id = ?", (entity_id,)
            ).fetchone()

            return Entity.from_row(row) if row else None

    def find_entity(
        self,
        entity_type: Optional[str] = None,
        name: Optional[str] = None,
    ) -> list[Entity]:
        """Find entities by type and/or name."""
        conditions = ["is_active = 1"]
        params: list[Any] = []

        if entity_type:
            conditions.append("entity_type = ?")
            params.append(entity_type)

        if name:
            conditions.append("(name LIKE ? OR aliases LIKE ?)")
            params.extend([f"%{name}%", f"%{name}%"])

        query = f"SELECT * FROM entities WHERE {' AND '.join(conditions)}"  # noqa: S608

        with self.connection() as conn:
            rows = conn.execute(query, params).fetchall()
            return [Entity.from_row(row) for row in rows]

    def _link_event_entity(
        self,
        conn: sqlite3.Connection,
        event_id: str,
        entity_id: str,
        role: Optional[str] = None,
    ) -> None:
        """Link an event to an entity (internal)."""
        conn.execute(
            """
            INSERT OR IGNORE INTO event_entity_links (id, event_id, entity_id, role)
            VALUES (?, ?, ?, ?)
            """,
            (generate_uuid(), event_id, entity_id, role),
        )

    def link_event_entity(
        self,
        event_id: str,
        entity_id: str,
        role: Optional[str] = None,
    ) -> None:
        """Link an event to an entity."""
        with self.connection() as conn:
            self._link_event_entity(conn, event_id, entity_id, role)

    # ========================================================================
    # P2P Sync: Mesh & Peers
    # ========================================================================

    def upsert_mesh(
        self,
        mesh_id: str,
        mesh_secret_hash: str,
        relay_url: Optional[str] = None,
    ) -> str:
        """
        Create or update a mesh network entry.

        Returns:
            The mesh_id
        """
        with self.connection() as conn:
            conn.execute(
                """
                INSERT INTO sync_mesh (mesh_id, mesh_secret_hash, relay_url)
                VALUES (?, ?, ?)
                ON CONFLICT(mesh_id) DO UPDATE SET
                    mesh_secret_hash = excluded.mesh_secret_hash,
                    relay_url = excluded.relay_url
                """,
                (mesh_id, mesh_secret_hash, relay_url),
            )
        return mesh_id

    def get_mesh(self, mesh_id: str) -> Optional[dict[str, Any]]:
        """Get a mesh network by ID."""
        with self.connection() as conn:
            row = conn.execute(
                "SELECT * FROM sync_mesh WHERE mesh_id = ?", (mesh_id,)
            ).fetchone()
            if row is None:
                return None
            return {
                "mesh_id": row["mesh_id"],
                "mesh_secret_hash": row["mesh_secret_hash"],
                "relay_url": row["relay_url"],
                "created_at": row["created_at"],
            }

    def list_meshes(self) -> list[dict[str, Any]]:
        """List all mesh networks."""
        with self.connection() as conn:
            rows = conn.execute(
                "SELECT * FROM sync_mesh ORDER BY created_at DESC"
            ).fetchall()
            return [
                {
                    "mesh_id": row["mesh_id"],
                    "mesh_secret_hash": row["mesh_secret_hash"],
                    "relay_url": row["relay_url"],
                    "created_at": row["created_at"],
                }
                for row in rows
            ]

    def delete_mesh(self, mesh_id: str) -> bool:
        """Delete a mesh network and its associated peers."""
        with self.connection() as conn:
            conn.execute(
                "DELETE FROM sync_peers WHERE mesh_id = ?", (mesh_id,)
            )
            cur = conn.execute(
                "DELETE FROM sync_mesh WHERE mesh_id = ?", (mesh_id,)
            )
            return cur.rowcount == 1

    def upsert_peer(
        self,
        peer_id: str,
        name: str,
        platform: str,
        mesh_id: str,
        connection_type: Optional[str] = None,
        address: Optional[str] = None,
    ) -> str:
        """
        Create or update a peer entry.

        Returns:
            The peer_id
        """
        with self.connection() as conn:
            conn.execute(
                """
                INSERT INTO sync_peers (peer_id, name, platform, mesh_id, connection_type, address)
                VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT(peer_id) DO UPDATE SET
                    name = excluded.name,
                    platform = excluded.platform,
                    mesh_id = excluded.mesh_id,
                    connection_type = excluded.connection_type,
                    address = excluded.address
                """,
                (peer_id, name, platform, mesh_id, connection_type, address),
            )
        return peer_id

    def get_peer(self, peer_id: str) -> Optional[dict[str, Any]]:
        """Get a peer by ID."""
        with self.connection() as conn:
            row = conn.execute(
                "SELECT * FROM sync_peers WHERE peer_id = ?", (peer_id,)
            ).fetchone()
            if row is None:
                return None
            return {
                "peer_id": row["peer_id"],
                "name": row["name"],
                "platform": row["platform"],
                "mesh_id": row["mesh_id"],
                "last_connected": row["last_connected"],
                "connection_type": row["connection_type"],
                "address": row["address"],
                "created_at": row["created_at"],
            }

    def list_peers(self, mesh_id: Optional[str] = None) -> list[dict[str, Any]]:
        """List peers, optionally filtered by mesh_id."""
        with self.connection() as conn:
            if mesh_id:
                rows = conn.execute(
                    "SELECT * FROM sync_peers WHERE mesh_id = ? ORDER BY created_at DESC",
                    (mesh_id,),
                ).fetchall()
            else:
                rows = conn.execute(
                    "SELECT * FROM sync_peers ORDER BY created_at DESC"
                ).fetchall()
            return [
                {
                    "peer_id": row["peer_id"],
                    "name": row["name"],
                    "platform": row["platform"],
                    "mesh_id": row["mesh_id"],
                    "last_connected": row["last_connected"],
                    "connection_type": row["connection_type"],
                    "address": row["address"],
                    "created_at": row["created_at"],
                }
                for row in rows
            ]

    def update_peer_connection(
        self,
        peer_id: str,
        connection_type: Optional[str] = None,
        address: Optional[str] = None,
    ) -> bool:
        """Update a peer's last_connected timestamp and optionally connection info."""
        with self.connection() as conn:
            now = utc_now()
            cur = conn.execute(
                """
                UPDATE sync_peers SET
                    last_connected = ?,
                    connection_type = COALESCE(?, connection_type),
                    address = COALESCE(?, address)
                WHERE peer_id = ?
                """,
                (now, connection_type, address, peer_id),
            )
            return cur.rowcount == 1

    def delete_peer(self, peer_id: str) -> bool:
        """Delete a peer."""
        with self.connection() as conn:
            cur = conn.execute(
                "DELETE FROM sync_peers WHERE peer_id = ?", (peer_id,)
            )
            return cur.rowcount == 1

    # ========================================================================
    # Import Tracking
    # ========================================================================

    def start_import(
        self,
        source: str,
        file_name: Optional[str] = None,
        file_hash: Optional[str] = None,
    ) -> str:
        """Start tracking an import operation."""
        import_id = generate_uuid()

        with self.connection() as conn:
            conn.execute(
                """
                INSERT INTO imports (id, source, file_name, file_hash, status)
                VALUES (?, ?, ?, ?, 'running')
                """,
                (import_id, source, file_name, file_hash),
            )

        return import_id

    def complete_import(
        self,
        import_id: str,
        records_created: int,
        records_updated: int = 0,
        records_skipped: int = 0,
        date_range_start: Optional[str] = None,
        date_range_end: Optional[str] = None,
    ) -> None:
        """Mark import as completed."""
        with self.connection() as conn:
            conn.execute(
                """
                UPDATE imports SET
                    status = 'completed',
                    records_total = ?,
                    records_created = ?,
                    records_updated = ?,
                    records_skipped = ?,
                    date_range_start = ?,
                    date_range_end = ?,
                    completed_at = ?
                WHERE id = ?
                """,
                (
                    records_created + records_updated + records_skipped,
                    records_created,
                    records_updated,
                    records_skipped,
                    date_range_start,
                    date_range_end,
                    utc_now(),
                    import_id,
                ),
            )

    def fail_import(self, import_id: str, error_message: str) -> None:
        """Mark import as failed."""
        with self.connection() as conn:
            conn.execute(
                """
                UPDATE imports SET
                    status = 'failed',
                    error_message = ?,
                    completed_at = ?
                WHERE id = ?
                """,
                (error_message, utc_now(), import_id),
            )

    # ========================================================================
    # Maintenance
    # ========================================================================

    def vacuum(self) -> None:
        """Reclaim unused space."""
        with self.connection() as conn:
            conn.execute("VACUUM")

    def analyze(self) -> None:
        """Update query planner statistics."""
        with self.connection() as conn:
            conn.execute("ANALYZE")

    def checkpoint(self, mode: str = "TRUNCATE") -> dict[str, int]:
        """
        Run WAL checkpoint.

        Useful before copying/moving the database file to ensure the main DB file
        contains the latest committed transactions and to reduce/clear the WAL.

        Args:
            mode: One of PASSIVE, FULL, RESTART, TRUNCATE (default: TRUNCATE).

        Returns:
            Dict with keys: busy, log, checkpointed.
        """
        mode = mode.upper()
        if mode not in {"PASSIVE", "FULL", "RESTART", "TRUNCATE"}:
            raise ValueError(f"Invalid checkpoint mode: {mode}")

        with self.connection() as conn:
            row = conn.execute(f"PRAGMA wal_checkpoint({mode})").fetchone()  # noqa: S608
            if row is None:
                return {"busy": 0, "log": 0, "checkpointed": 0}
            return {
                "busy": int(row[0]),
                "log": int(row[1]),
                "checkpointed": int(row[2]),
            }

    def backup_to(
        self,
        output_path: Union[str, Path],
        overwrite: bool = False,
    ) -> Path:
        """
        Create a consistent backup of the database using SQLite's online backup API.

        This is the safest way to make a portable copy of a database (especially in WAL mode).
        """
        output_path = Path(output_path)
        if output_path.exists():
            if output_path.is_dir():
                raise IsADirectoryError(f"Backup path is a directory: {output_path}")
            if not overwrite:
                raise FileExistsError(f"Backup file already exists: {output_path}")
            output_path.unlink()
        output_path.parent.mkdir(parents=True, exist_ok=True)

        with self.connection() as src:
            dest = sqlite3.connect(output_path)
            try:
                src.backup(dest)
                dest.commit()
            finally:
                dest.close()

        return output_path

    def integrity_check(self) -> bool:
        """Check database integrity."""
        with self.connection() as conn:
            result = conn.execute("PRAGMA integrity_check").fetchone()
            return bool(result is not None and result[0] == "ok")

    def get_stats(self) -> dict[str, Any]:
        """Get database statistics."""
        with self.connection() as conn:
            stats: dict[str, Any] = {
                "file_size_bytes": self.db_path.stat().st_size,
                "events_count": conn.execute(
                    "SELECT COUNT(*) FROM events WHERE is_deleted = 0"
                ).fetchone()[0],
                "entities_count": conn.execute(
                    "SELECT COUNT(*) FROM entities WHERE is_active = 1"
                ).fetchone()[0],
                "event_types": {},
                "categories": {},
            }

            for row in conn.execute(
                """
                SELECT event_type, COUNT(*) as count
                FROM events WHERE is_deleted = 0
                GROUP BY event_type
                """
            ):
                stats["event_types"][row["event_type"]] = row["count"]

            for row in conn.execute(
                """
                SELECT category, COUNT(*) as count
                FROM events WHERE is_deleted = 0 AND category IS NOT NULL
                GROUP BY category
                """
            ):
                stats["categories"][row["category"]] = row["count"]

            return stats

    def export_to_json(
        self,
        output_path: Union[str, Path],
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> int:
        """Export events to JSON file."""
        events = self.query_events(
            start_date=start_date,
            end_date=end_date,
            limit=10_000_000,
        )

        output = {
            "exported_at": utc_now(),
            "count": len(events),
            "events": [asdict(e) for e in events],
        }

        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(output, f, ensure_ascii=False, indent=2)

        return len(events)


# ============================================================================
# CLI Interface
# ============================================================================


def main() -> None:
    """Simple CLI for database operations."""
    import sys

    if len(sys.argv) < 2:
        print("Usage: python3 ark.py <database.db> [command]")
        print("Commands: stats, check, vacuum, checkpoint, backup")
        print("")
        print("Examples:")
        print("  python3 ark.py life.db stats")
        print("  python3 ark.py life.db check")
        print(
            "  python3 ark.py life.db checkpoint [PASSIVE|FULL|RESTART|TRUNCATE]"
        )
        print("  python3 ark.py life.db backup life-backup.db")
        return

    db_path = sys.argv[1]
    command = sys.argv[2] if len(sys.argv) > 2 else "stats"

    db = Ark(db_path)

    if command == "stats":
        stats = db.get_stats()
        print(f"Database: {db_path}")
        print(f"Size: {stats['file_size_bytes'] / 1024 / 1024:.2f} MB")
        print(f"Events: {stats['events_count']:,}")
        print(f"Entities: {stats['entities_count']:,}")
        if stats["event_types"]:
            print("Event types:")
            for et, count in sorted(stats["event_types"].items()):
                print(f"  {et}: {count:,}")
        if stats["categories"]:
            print("Categories:")
            for cat, count in sorted(stats["categories"].items()):
                print(f"  {cat}: {count:,}")

    elif command == "check":
        if db.integrity_check():
            print("Database integrity: OK")
        else:
            print("Database integrity: FAILED")
            sys.exit(1)

    elif command == "vacuum":
        print("Running VACUUM...")
        db.vacuum()
        print("Done")

    elif command == "checkpoint":
        mode = sys.argv[3] if len(sys.argv) > 3 else "TRUNCATE"
        result = db.checkpoint(mode=mode)
        print(f"WAL checkpoint ({mode.upper()}): {result}")

    elif command == "backup":
        if len(sys.argv) < 4:
            print("Usage: python3 ark.py <database.db> backup <output.db>")
            sys.exit(2)
        output_path = sys.argv[3]
        out = db.backup_to(output_path, overwrite=False)
        print(f"Backup written to: {out}")

    else:
        print(f"Unknown command: {command}")


if __name__ == "__main__":
    main()
