"""
Tests for Ark Core.

Run from repo root:
    python3 -m core.tests.test_ark
"""

from __future__ import annotations

import json
import sqlite3
import sys
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

from core.ark import (
    Ark,
    Entity,
    Event,
    generate_uuid,
    parse_iso8601,
    to_iso8601,
    utc_now,
)


class TestUtilityFunctions(unittest.TestCase):
    """Test utility functions."""

    def test_generate_uuid(self) -> None:
        """UUID should be valid format."""
        uid = generate_uuid()
        assert len(uid) == 36
        assert uid.count("-") == 4
        uid2 = generate_uuid()
        assert uid != uid2

    def test_generate_uuid_uniqueness(self) -> None:
        """Multiple UUIDs should all be unique."""
        uuids = [generate_uuid() for _ in range(1000)]
        assert len(set(uuids)) == 1000

    def test_utc_now(self) -> None:
        """utc_now should return valid ISO 8601 timestamp."""
        ts = utc_now()
        assert ts.endswith("Z")
        assert "T" in ts
        dt = parse_iso8601(ts)
        assert dt is not None
        assert dt.tzinfo == timezone.utc

    def test_to_iso8601_naive(self) -> None:
        """to_iso8601 should handle naive datetime."""
        dt = datetime(2024, 1, 15, 10, 30, 0, tzinfo=timezone.utc)
        result = to_iso8601(dt)
        assert result == "2024-01-15T10:30:00Z"

    def test_to_iso8601_aware(self) -> None:
        """to_iso8601 should handle timezone-aware datetime."""
        dt = datetime(2024, 1, 15, 10, 30, 0, tzinfo=timezone.utc)
        result = to_iso8601(dt)
        assert result == "2024-01-15T10:30:00Z"

    def test_parse_iso8601_formats(self) -> None:
        """parse_iso8601 should handle various ISO 8601 formats."""
        test_cases = [
            (
                "2024-01-15T10:30:00Z",
                datetime(2024, 1, 15, 10, 30, 0, tzinfo=timezone.utc),
            ),
            (
                "2024-01-15T10:30:00",
                datetime(2024, 1, 15, 10, 30, 0, tzinfo=timezone.utc),
            ),
            (
                "2024-01-15T10:30:00.123Z",
                datetime(2024, 1, 15, 10, 30, 0, 123000, tzinfo=timezone.utc),
            ),
        ]
        for input_str, expected in test_cases:
            with self.subTest(input_str=input_str):
                result = parse_iso8601(input_str)
                assert result.year == expected.year
                assert result.month == expected.month
                assert result.day == expected.day

    def test_parse_iso8601_invalid(self) -> None:
        """parse_iso8601 should raise ValueError for invalid input."""
        with self.assertRaises(ValueError):
            parse_iso8601("not a timestamp")

    def test_parse_iso8601_empty(self) -> None:
        """parse_iso8601 should raise ValueError for empty string."""
        with self.assertRaises(ValueError):
            parse_iso8601("")


class TestArk(unittest.TestCase):
    """Test Ark class."""

    def setUp(self) -> None:
        """Create temporary database for each test."""
        self.temp_dir = tempfile.mkdtemp()
        self.db_path = Path(self.temp_dir) / "test.db"
        self.db = Ark(self.db_path, create=True)

    def tearDown(self) -> None:
        """Clean up temporary files."""
        import shutil

        shutil.rmtree(self.temp_dir, ignore_errors=True)

    # ========================================================================
    # Event Recording Tests
    # ========================================================================

    def test_record_event_basic(self) -> None:
        """Should record a basic event."""
        event_id = self.db.record_event(
            "heart_rate",
            {"bpm": 72},
            summary="Test heart rate",
        )
        assert event_id is not None
        assert len(event_id) == 36

    def test_record_event_with_category(self) -> None:
        """Should record event with category."""
        event_id = self.db.record_event(
            "heart_rate",
            {"bpm": 72},
            category="health",
            summary="Test",
        )
        event = self.db.get_event(event_id)
        assert event is not None
        assert event.category == "health"

    def test_record_event_with_all_fields(self) -> None:
        """Should record event with all optional fields."""
        event_id = self.db.record_event(
            "heart_rate",
            {"bpm": 72, "source": "test"},
            category="health",
            occurred_at="2024-01-15T10:30:00Z",
            summary="Full test event",
            duration_seconds=60,
            timezone="UTC",
            source="test_source",
            source_id="test_123",
            device="test_device",
            tags=["test", "automated"],
        )

        event = self.db.get_event(event_id)
        assert event is not None
        assert event.event_type == "heart_rate"
        assert event.data["bpm"] == 72
        assert event.summary == "Full test event"
        assert event.duration_seconds == 60
        assert event.source == "test_source"
        assert event.tags == ["test", "automated"]

    def test_record_event_upsert_by_source_id(self) -> None:
        """record_event should update existing event for the same source/source_id."""
        event_id1 = self.db.record_event(
            "note",
            {"content": "v1"},
            source="test",
            source_id="abc",
            summary="v1",
        )
        event_id2 = self.db.record_event(
            "note",
            {"content": "v2"},
            source="test",
            source_id="abc",
            summary="v2",
        )

        assert event_id1 == event_id2
        ev = self.db.get_event(event_id1)
        assert ev is not None
        assert ev.data["content"] == "v2"
        assert ev.summary == "v2"

    def test_record_event_datetime_occurred_at(self) -> None:
        """Should handle datetime object for occurred_at."""
        dt = datetime(2024, 1, 15, 10, 30, 0, tzinfo=timezone.utc)
        event_id = self.db.record_event(
            "heart_rate",
            {"bpm": 72},
            occurred_at=dt,
        )

        event = self.db.get_event(event_id)
        assert event is not None
        assert event.occurred_at == "2024-01-15T10:30:00Z"

    def test_record_event_any_type(self) -> None:
        """Should accept any event type (no validation in core)."""
        event_id = self.db.record_event("custom_event_type", {"data": 1})
        event = self.db.get_event(event_id)
        assert event is not None
        assert event.event_type == "custom_event_type"

    def test_record_events_batch(self) -> None:
        """Should record multiple events in batch."""
        events = [
            {"event_type": "heart_rate", "data": {"bpm": 70}},
            {"event_type": "heart_rate", "data": {"bpm": 72}},
            {"event_type": "heart_rate", "data": {"bpm": 75}},
        ]

        created, updated, skipped = self.db.record_events_batch(events)

        assert created == 3
        assert updated == 0
        assert skipped == 0

    def test_record_events_batch_deduplication(self) -> None:
        """Should update duplicate events by source/source_id."""
        events = [
            {
                "event_type": "heart_rate",
                "data": {"bpm": 70},
                "source": "test",
                "source_id": "1",
            },
            {
                "event_type": "heart_rate",
                "data": {"bpm": 72},  # update
                "source": "test",
                "source_id": "1",
            },
        ]

        created, updated, skipped = self.db.record_events_batch(events)

        assert created == 1
        assert updated == 1
        assert skipped == 0

        # Verify updated data was applied
        results = self.db.query_events(source="test", limit=10)
        assert len(results) == 1
        assert results[0].data["bpm"] == 72

    def test_record_events_batch_empty(self) -> None:
        """Should handle empty batch."""
        created, updated, skipped = self.db.record_events_batch([])
        assert created == 0
        assert updated == 0
        assert skipped == 0

    def test_record_events_batch_missing_event_type(self) -> None:
        """Should skip events without event_type."""
        events = [
            {"data": {"bpm": 70}},  # missing event_type
            {"event_type": "heart_rate", "data": {"bpm": 72}},
        ]

        created, updated, skipped = self.db.record_events_batch(events)

        assert created == 1
        assert updated == 0
        assert skipped == 1

    # ========================================================================
    # Query Tests
    # ========================================================================

    def test_query_events_empty(self) -> None:
        """Should return empty list for no matches."""
        events = self.db.query_events(category="nonexistent")
        assert events == []

    def test_query_events_by_type(self) -> None:
        """Should filter by event type."""
        self.db.record_event("heart_rate", {"bpm": 72})
        self.db.record_event("sleep", {"duration": 420})

        hr_events = self.db.query_events(event_type="heart_rate")
        assert len(hr_events) == 1
        assert hr_events[0].event_type == "heart_rate"

    def test_query_events_by_category(self) -> None:
        """Should filter by category."""
        self.db.record_event("heart_rate", {"bpm": 72}, category="health")
        self.db.record_event("meal", {"type": "lunch"}, category="nutrition")

        health_events = self.db.query_events(category="health")
        assert len(health_events) == 1
        assert health_events[0].category == "health"

    def test_query_events_by_date_range(self) -> None:
        """Should filter by date range."""
        self.db.record_event(
            "heart_rate", {"bpm": 70}, occurred_at="2024-01-01T10:00:00Z"
        )
        self.db.record_event("heart_rate", {"bpm": 72}, occurred_at=utc_now())

        recent = self.db.query_events(start_date="2024-01-10T00:00:00Z")
        assert len(recent) == 1

    def test_query_events_by_days(self) -> None:
        """Should filter by last N days."""
        old_date = datetime.now(timezone.utc) - timedelta(days=10)
        self.db.record_event("heart_rate", {"bpm": 70}, occurred_at=old_date)
        self.db.record_event("heart_rate", {"bpm": 72})

        recent = self.db.query_events(days=7)
        assert len(recent) == 1

    def test_query_events_order(self) -> None:
        """Should respect order parameter."""
        self.db.record_event(
            "heart_rate", {"bpm": 70}, occurred_at="2024-01-01T10:00:00Z"
        )
        self.db.record_event(
            "heart_rate", {"bpm": 72}, occurred_at="2024-01-02T10:00:00Z"
        )

        desc = self.db.query_events(order="DESC")
        asc = self.db.query_events(order="ASC")

        assert desc[0].data["bpm"] == 72
        assert asc[0].data["bpm"] == 70

    def test_query_events_invalid_order(self) -> None:
        """Should raise error for invalid order."""
        with self.assertRaises(ValueError) as ctx:
            self.db.query_events(order="INVALID")
        assert "Invalid order" in str(ctx.exception)

    def test_query_events_limit_offset(self) -> None:
        """Should respect limit and offset."""
        for i in range(10):
            self.db.record_event("heart_rate", {"bpm": 70 + i})

        page1 = self.db.query_events(limit=3, offset=0)
        page2 = self.db.query_events(limit=3, offset=3)

        assert len(page1) == 3
        assert len(page2) == 3
        assert page1[0].id != page2[0].id

    def test_query_events_by_source(self) -> None:
        """Should filter by source."""
        self.db.record_event("heart_rate", {"bpm": 72}, source="garmin")
        self.db.record_event("heart_rate", {"bpm": 75}, source="manual")

        garmin_events = self.db.query_events(source="garmin")
        assert len(garmin_events) == 1

    def test_query_events_by_tags(self) -> None:
        """Should filter by tags."""
        self.db.record_event("heart_rate", {"bpm": 72}, tags=["morning", "rest"])
        self.db.record_event("heart_rate", {"bpm": 75}, tags=["exercise"])

        morning_events = self.db.query_events(tags=["morning"])
        assert len(morning_events) == 1

    def test_count_events(self) -> None:
        """Should count events correctly."""
        self.db.record_event("heart_rate", {"bpm": 72})
        self.db.record_event("heart_rate", {"bpm": 75})
        self.db.record_event("sleep", {"duration": 420})

        total = self.db.count_events()
        hr_count = self.db.count_events(event_type="heart_rate")

        assert total == 3
        assert hr_count == 2

    def test_get_event_not_found(self) -> None:
        """Should return None for non-existent event."""
        event = self.db.get_event("nonexistent-id")
        assert event is None

    def test_delete_event(self) -> None:
        """Should soft-delete event."""
        event_id = self.db.record_event("note", {"content": "test"})
        assert self.db.get_event(event_id) is not None

        ok = self.db.delete_event(event_id)
        assert ok is True

        assert self.db.get_event(event_id) is None
        assert self.db.count_events() == 0

    # ========================================================================
    # Full-Text Search Tests
    # ========================================================================

    def test_search_basic(self) -> None:
        """Should find events by text search."""
        self.db.record_event(
            "note", {"content": "headache pain"}, summary="Headache note"
        )
        self.db.record_event(
            "note", {"content": "Happy thoughts"}, summary="Happy note"
        )

        results = self.db.search("headache")
        assert len(results) == 1

    def test_search_no_results(self) -> None:
        """Should return empty for no matches."""
        self.db.record_event("note", {"content": "test"}, summary="test")

        results = self.db.search("nonexistent_word_xyz")
        assert len(results) == 0

    def test_search_with_limit(self) -> None:
        """Should respect limit parameter."""
        for i in range(10):
            self.db.record_event(
                "note", {"content": f"coding {i}"}, summary=f"Coding note {i}"
            )

        results = self.db.search("coding", limit=3)
        assert len(results) == 3

    # ========================================================================
    # Entity Tests
    # ========================================================================

    def test_create_entity(self) -> None:
        """Should create entity."""
        entity_id = self.db.create_entity(
            "person",
            "John Doe",
            data={"email": "john@example.com"},
            aliases=["Johnny", "JD"],
        )

        entity = self.db.get_entity(entity_id)
        assert entity is not None
        assert entity.name == "John Doe"
        assert entity.entity_type == "person"
        assert entity.data["email"] == "john@example.com"
        assert entity.aliases == ["Johnny", "JD"]

    def test_create_entity_minimal(self) -> None:
        """Should create entity with minimal data."""
        entity_id = self.db.create_entity("place", "Home")

        entity = self.db.get_entity(entity_id)
        assert entity is not None
        assert entity.name == "Home"

    def test_find_entity_by_type(self) -> None:
        """Should find entities by type."""
        self.db.create_entity("person", "John")
        self.db.create_entity("person", "Jane")
        self.db.create_entity("place", "Office")

        people = self.db.find_entity(entity_type="person")
        assert len(people) == 2

    def test_find_entity_by_name(self) -> None:
        """Should find entities by name substring."""
        self.db.create_entity("person", "John Doe")
        self.db.create_entity("person", "Jane Doe")
        self.db.create_entity("person", "Bob Smith")

        does = self.db.find_entity(name="Doe")
        assert len(does) == 2

    def test_get_entity_not_found(self) -> None:
        """Should return None for non-existent entity."""
        entity = self.db.get_entity("nonexistent-id")
        assert entity is None

    def test_link_event_entity(self) -> None:
        """Should link events to entities."""
        entity_id = self.db.create_entity("person", "Dr. Smith")
        event_id = self.db.record_event(
            "doctor_visit", {"reason": "checkup"}, entity_ids=[entity_id]
        )

        with self.db.connection() as conn:
            link = conn.execute(
                "SELECT * FROM event_entity_links WHERE event_id = ?",
                (event_id,),
            ).fetchone()
            assert link is not None
            assert link["entity_id"] == entity_id

    def test_link_event_multiple_entities(self) -> None:
        """Should link event to multiple entities."""
        entity1 = self.db.create_entity("person", "Dr. Smith")
        entity2 = self.db.create_entity("place", "Hospital")
        event_id = self.db.record_event(
            "doctor_visit", {"reason": "checkup"}, entity_ids=[entity1, entity2]
        )

        with self.db.connection() as conn:
            links = conn.execute(
                "SELECT * FROM event_entity_links WHERE event_id = ?",
                (event_id,),
            ).fetchall()
            assert len(links) == 2

    # ========================================================================
    # Statistics Tests
    # ========================================================================

    def test_get_daily_stats(self) -> None:
        """Should return daily statistics."""
        today = datetime.now(timezone.utc).strftime("%Y-%m-%d")

        self.db.record_event("heart_rate", {"bpm": 72})
        self.db.record_event("heart_rate", {"bpm": 75})
        self.db.record_event("sleep", {"duration": 420})

        stats = self.db.get_daily_stats(today)

        assert stats["date"] == today
        assert stats["total_events"] == 3
        assert stats["by_type"]["heart_rate"] == 2

    def test_get_daily_stats_no_events(self) -> None:
        """Should return zero counts for day with no events."""
        stats = self.db.get_daily_stats("2020-01-01")
        assert stats["total_events"] == 0

    def test_get_stats(self) -> None:
        """Should return database statistics."""
        self.db.record_event("heart_rate", {"bpm": 72}, category="health")
        self.db.create_entity("person", "Test")

        stats = self.db.get_stats()

        assert "file_size_bytes" in stats
        assert "events_count" in stats
        assert "entities_count" in stats
        assert "event_types" in stats
        assert "categories" in stats
        assert stats["events_count"] == 1
        assert stats["entities_count"] == 1

    # ========================================================================
    # Maintenance Tests
    # ========================================================================

    def test_integrity_check(self) -> None:
        """Should pass integrity check."""
        result = self.db.integrity_check()
        assert result is True

    def test_vacuum(self) -> None:
        """Should run vacuum without error."""
        self.db.record_event("heart_rate", {"bpm": 72})
        self.db.vacuum()

    def test_analyze(self) -> None:
        """Should run analyze without error."""
        self.db.record_event("heart_rate", {"bpm": 72})
        self.db.analyze()

    def test_checkpoint(self) -> None:
        """Should run WAL checkpoint without error."""
        self.db.record_event("heart_rate", {"bpm": 72})
        result = self.db.checkpoint()
        assert set(result.keys()) == {"busy", "log", "checkpointed"}

    def test_backup_to(self) -> None:
        """Should create a consistent backup using SQLite backup API."""
        self.db.record_event("heart_rate", {"bpm": 72})
        backup_path = Path(self.temp_dir) / "backup.db"
        out = self.db.backup_to(backup_path)
        assert out == backup_path
        assert backup_path.exists()

        conn = sqlite3.connect(backup_path)
        count = conn.execute(
            "SELECT COUNT(*) FROM events WHERE is_deleted = 0"
        ).fetchone()[0]
        assert count == 1
        conn.close()

    # ========================================================================
    # Import Tracking Tests
    # ========================================================================

    def test_import_tracking(self) -> None:
        """Should track import operations."""
        import_id = self.db.start_import("test_source", "test.csv", "abc123")

        self.db.complete_import(
            import_id,
            records_created=100,
            records_updated=10,
            records_skipped=5,
            date_range_start="2024-01-01",
            date_range_end="2024-01-31",
        )

        with self.db.connection() as conn:
            record = conn.execute(
                "SELECT * FROM imports WHERE id = ?", (import_id,)
            ).fetchone()
            assert record["status"] == "completed"
            assert record["records_created"] == 100

    def test_import_failure(self) -> None:
        """Should track failed imports."""
        import_id = self.db.start_import("test_source")
        self.db.fail_import(import_id, "Test error message")

        with self.db.connection() as conn:
            record = conn.execute(
                "SELECT * FROM imports WHERE id = ?", (import_id,)
            ).fetchone()
            assert record["status"] == "failed"
            assert record["error_message"] == "Test error message"

    # ========================================================================
    # Export Tests
    # ========================================================================

    def test_export_to_json(self) -> None:
        """Should export events to JSON."""
        self.db.record_event("heart_rate", {"bpm": 72}, summary="Test")
        self.db.record_event("heart_rate", {"bpm": 75}, summary="Test 2")

        output_path = Path(self.temp_dir) / "export.json"
        count = self.db.export_to_json(output_path)

        assert count == 2
        assert output_path.exists()

        with open(output_path) as f:
            data = json.load(f)

        assert data["count"] == 2
        assert len(data["events"]) == 2


class TestSchemaIntegrity(unittest.TestCase):
    """Test database schema integrity."""

    def setUp(self) -> None:
        """Create temporary database."""
        self.temp_dir = tempfile.mkdtemp()
        self.db_path = Path(self.temp_dir) / "test.db"
        self.schema_path = Path(__file__).parent.parent / "schema.sql"

    def tearDown(self) -> None:
        """Clean up."""
        import shutil

        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_schema_creates_all_tables(self) -> None:
        """Schema should create all required tables."""
        conn = sqlite3.connect(self.db_path)
        with open(self.schema_path) as f:
            conn.executescript(f.read())

        cursor = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
        )
        tables = {row[0] for row in cursor.fetchall()}

        required_tables = {
            "events",
            "entities",
            "event_entity_links",
            "embeddings",
            "imports",
            "metadata",
            "events_fts",
        }

        for table in required_tables:
            assert table in tables, f"Missing table: {table}"

        conn.close()

    def test_schema_creates_indexes(self) -> None:
        """Schema should create required indexes."""
        conn = sqlite3.connect(self.db_path)
        with open(self.schema_path) as f:
            conn.executescript(f.read())

        cursor = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
        indexes = {row[0] for row in cursor.fetchall()}

        assert "idx_events_occurred_at" in indexes
        assert "idx_events_event_type" in indexes

        conn.close()

    def test_timestamp_validation_trigger(self) -> None:
        """Trigger should validate timestamp format."""
        conn = sqlite3.connect(self.db_path)
        with open(self.schema_path) as f:
            conn.executescript(f.read())

        with self.assertRaises(sqlite3.IntegrityError):
            conn.execute(
                """
                INSERT INTO events (id, event_type, occurred_at, data)
                VALUES ('test', 'heart_rate', 'invalid-timestamp', '{}')
                """
            )

        conn.close()

    def test_wal_mode_enabled(self) -> None:
        """WAL mode should be enabled."""
        conn = sqlite3.connect(self.db_path)
        with open(self.schema_path) as f:
            conn.executescript(f.read())

        result = conn.execute("PRAGMA journal_mode").fetchone()
        assert result[0] == "wal"

        conn.close()

    def test_schema_version(self) -> None:
        """Schema version should be recorded in metadata."""
        conn = sqlite3.connect(self.db_path)
        with open(self.schema_path) as f:
            conn.executescript(f.read())

        result = conn.execute(
            "SELECT value FROM metadata WHERE key = 'schema_version'"
        ).fetchone()
        assert result is not None
        assert result[0] == "2.0.0"

        conn.close()


class TestKeepAlive(unittest.TestCase):
    """Test keep_alive connection pooling."""

    def setUp(self) -> None:
        self.temp_dir = tempfile.mkdtemp()
        self.db_path = Path(self.temp_dir) / "test_ka.db"

    def tearDown(self) -> None:
        import shutil

        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_keep_alive_basic_operations(self) -> None:
        """keep_alive=True should work for normal CRUD."""
        db = Ark(self.db_path, keep_alive=True)
        eid = db.record_event("hr", {"bpm": 72}, summary="test")
        event = db.get_event(eid)
        assert event is not None
        assert event.data["bpm"] == 72
        db.close()

    def test_keep_alive_reuses_connection(self) -> None:
        """Persistent mode should reuse the same connection object."""
        db = Ark(self.db_path, keep_alive=True)
        with db.connection() as c1:
            pass
        with db.connection() as c2:
            pass
        assert c1 is c2
        db.close()

    def test_default_does_not_reuse(self) -> None:
        """Default mode should use a fresh connection each time."""
        db = Ark(self.db_path, keep_alive=False)
        with db.connection() as c1:
            pass
        with db.connection() as c2:
            pass
        assert c1 is not c2

    def test_context_manager(self) -> None:
        """Ark should work as a context manager and close on exit."""
        with Ark(self.db_path, keep_alive=True) as db:
            db.record_event("note", {"text": "hello"})
            # Force connection creation
            with db.connection() as conn:
                pass
        # After exiting, the persistent connection should be closed
        assert getattr(db._local, "conn", None) is None

    def test_close_idempotent(self) -> None:
        """Calling close() multiple times should not raise."""
        db = Ark(self.db_path, keep_alive=True)
        db.record_event("note", {"text": "hi"})
        db.close()
        db.close()  # second call should be safe

    def test_close_noop_without_keep_alive(self) -> None:
        """close() should be a no-op when keep_alive=False."""
        db = Ark(self.db_path, keep_alive=False)
        db.record_event("note", {"text": "hi"})
        db.close()  # should not raise

    def test_keep_alive_thread_safety(self) -> None:
        """Each thread should get its own connection."""
        import threading

        db = Ark(self.db_path, keep_alive=True)
        connections: list = []
        barrier = threading.Barrier(2)

        def worker() -> None:
            with db.connection() as conn:
                connections.append(conn)
                barrier.wait()

        t1 = threading.Thread(target=worker)
        t2 = threading.Thread(target=worker)
        t1.start()
        t2.start()
        t1.join()
        t2.join()

        assert len(connections) == 2
        assert connections[0] is not connections[1]
        db.close()

    def test_keep_alive_rollback_on_error(self) -> None:
        """Errors inside keep_alive connection should rollback but keep conn alive."""
        db = Ark(self.db_path, keep_alive=True)
        db.record_event("note", {"text": "before"})

        with self.assertRaises(sqlite3.OperationalError):
            with db.connection() as conn:
                conn.execute("SELECT * FROM nonexistent_table_xyz")

        # Connection should still be usable
        eid = db.record_event("note", {"text": "after"})
        assert db.get_event(eid) is not None
        db.close()


class TestDataclasses(unittest.TestCase):
    """Test Event and Entity dataclasses."""

    def test_event_from_row(self) -> None:
        """Should create Event from sqlite Row."""
        conn = sqlite3.connect(":memory:")
        conn.row_factory = sqlite3.Row
        conn.execute("""
            CREATE TABLE test (
                id TEXT, event_type TEXT, category TEXT, occurred_at TEXT,
                data TEXT, summary TEXT, duration_seconds INTEGER, timezone TEXT,
                source TEXT, source_id TEXT, device TEXT, tags TEXT,
                created_at TEXT, updated_at TEXT
            )
        """)
        conn.execute("""
            INSERT INTO test VALUES (
                'id1', 'heart_rate', 'health', '2024-01-01T00:00:00Z',
                '{"bpm": 72}', 'Test', 60, 'UTC',
                'manual', 'src1', 'phone', '["tag1"]',
                '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'
            )
        """)
        row = conn.execute("SELECT * FROM test").fetchone()

        event = Event.from_row(row)

        assert event.id == "id1"
        assert event.event_type == "heart_rate"
        assert event.data == {"bpm": 72}
        assert event.tags == ["tag1"]

    def test_event_from_row_with_null_tags(self) -> None:
        """Should handle null tags."""
        conn = sqlite3.connect(":memory:")
        conn.row_factory = sqlite3.Row
        conn.execute("""
            CREATE TABLE test (
                id TEXT, event_type TEXT, category TEXT, occurred_at TEXT,
                data TEXT, summary TEXT, duration_seconds INTEGER, timezone TEXT,
                source TEXT, source_id TEXT, device TEXT, tags TEXT,
                created_at TEXT, updated_at TEXT
            )
        """)
        conn.execute("""
            INSERT INTO test VALUES (
                'id1', 'heart_rate', NULL, '2024-01-01T00:00:00Z',
                '{"bpm": 72}', 'Test', NULL, NULL,
                NULL, NULL, NULL, NULL,
                '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'
            )
        """)
        row = conn.execute("SELECT * FROM test").fetchone()

        event = Event.from_row(row)

        assert event.tags == []
        assert event.category is None

    def test_entity_from_row(self) -> None:
        """Should create Entity from sqlite Row."""
        conn = sqlite3.connect(":memory:")
        conn.row_factory = sqlite3.Row
        conn.execute("""
            CREATE TABLE test (
                id TEXT, entity_type TEXT, name TEXT, data TEXT,
                aliases TEXT, is_active INTEGER, created_at TEXT, updated_at TEXT
            )
        """)
        conn.execute("""
            INSERT INTO test VALUES (
                'id1', 'person', 'John', '{"email": "john@test.com"}',
                '["Johnny"]', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'
            )
        """)
        row = conn.execute("SELECT * FROM test").fetchone()

        entity = Entity.from_row(row)

        assert entity.id == "id1"
        assert entity.name == "John"
        assert entity.data == {"email": "john@test.com"}
        assert entity.aliases == ["Johnny"]
        assert entity.is_active is True


def run_tests() -> None:
    """Run all tests and print report."""
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()

    suite.addTests(loader.loadTestsFromTestCase(TestUtilityFunctions))
    suite.addTests(loader.loadTestsFromTestCase(TestArk))
    suite.addTests(loader.loadTestsFromTestCase(TestSchemaIntegrity))
    suite.addTests(loader.loadTestsFromTestCase(TestKeepAlive))
    suite.addTests(loader.loadTestsFromTestCase(TestDataclasses))

    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)

    print("\n" + "=" * 70)
    print(f"Tests run: {result.testsRun}")
    print(f"Failures: {len(result.failures)}")
    print(f"Errors: {len(result.errors)}")

    if result.wasSuccessful():
        print("\nALL TESTS PASSED")
    else:
        print("\nSOME TESTS FAILED")
        sys.exit(1)


if __name__ == "__main__":
    run_tests()
