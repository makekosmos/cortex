# Ark Core

Minimal event storage for personal life data. One SQLite file, zero dependencies.

## Overview

Ark Core is the foundation layer — a "dumb" storage engine that stores all life events in a single SQLite file. It has no predefined categories, event types, or business logic.

**All semantics come from plugins** (see [plugins](../plugins)).

**Core provides:**
- Event storage with flexible JSON data
- Entity management (people, places, etc.)
- Full-text search (FTS5)
- Import tracking & deduplication
- Vector embeddings support (sqlite-vec ready)

## Quick Start

```python
from core import Ark

db = Ark("life.db")

# Record any event (no predefined types)
db.record_event("heart_rate", {"bpm": 72}, category="health")
db.record_event("video_watched", {"title": "...", "url": "..."}, category="media")
db.record_event("custom_event", {"anything": "goes"})

# Query
events = db.query_events(category="health", days=7)
events = db.query_events(event_type="heart_rate")

# Full-text search
results = db.search("headache")

# Entities
person_id = db.create_entity("person", "John Doe")
db.record_event("meeting", {"notes": "..."}, entity_ids=[person_id])
```

## Project Structure

```
core/
├── ark.py    # Python library (~400 lines)
├── schema.sql      # Database schema (~200 lines)
├── tests/
│   └── test_ark.py
└── README.md
```

## Database Schema

| Table | Purpose |
|-------|---------|
| `events` | All events (main table) |
| `entities` | Reference data (people, places, etc.) |
| `event_entity_links` | Event-entity relationships |
| `embeddings` | Vector embeddings for semantic search |
| `imports` | Import tracking for deduplication |
| `metadata` | Schema version |
| `events_fts` | Full-text search index |

## API Reference

### Recording Events

```python
# Basic
event_id = db.record_event("event_type", {"data": "here"})

# With all options
event_id = db.record_event(
    "heart_rate",
    {"bpm": 72},
    category="health",           # Optional grouping
    occurred_at="2024-01-15T10:30:00Z",  # Default: now
    summary="HR: 72 bpm",        # For search
    duration_seconds=60,
    timezone="Europe/Moscow",
    source="garmin",             # For deduplication
    source_id="12345",
    device="Garmin Watch",
    tags=["morning", "rest"],
    entity_ids=[person_id],      # Link to entities
)

# Batch insert
created, updated, skipped = db.record_events_batch([
    {"event_type": "heart_rate", "data": {"bpm": 70}},
    {"event_type": "heart_rate", "data": {"bpm": 72}},
])
```

### Querying Events

```python
# All filters are optional
events = db.query_events(
    event_type="heart_rate",
    category="health",
    start_date="2024-01-01T00:00:00Z",
    end_date="2024-01-31T23:59:59Z",
    days=7,                      # Alternative to date range
    source="garmin",
    tags=["morning"],
    search="keyword",            # Full-text search
    limit=100,
    offset=0,
    order="DESC",                # or "ASC"
)

# Count
count = db.count_events(event_type="heart_rate", days=30)

# Get single event
event = db.get_event(event_id)
```

### Entities

```python
# Create
entity_id = db.create_entity(
    "person",
    "John Doe",
    data={"email": "john@example.com"},
    aliases=["Johnny"],
)

# Find
entities = db.find_entity(entity_type="person", name="John")

# Link to event
db.link_event_entity(event_id, entity_id, role="participant")
```

### Import Tracking

```python
import_id = db.start_import("garmin", "export.zip", file_hash="abc123")

# ... import events ...

db.complete_import(import_id, records_created=100, records_skipped=5)
# or
db.fail_import(import_id, "Error message")
```

### Maintenance

```python
db.vacuum()          # Reclaim space
db.analyze()         # Update query planner
db.integrity_check() # Verify database
db.get_stats()       # Get counts and sizes
db.export_to_json("backup.json")

# One-file portable backup (safe even in WAL mode)
db.backup_to("life-backup.db")

# Reduce/clear WAL before copying the original file
db.checkpoint()
```

## CLI

```bash
python3 ark.py life.db stats
python3 ark.py life.db check
python3 ark.py life.db vacuum
python3 ark.py life.db checkpoint TRUNCATE
python3 ark.py life.db backup life-backup.db
```

## SQL Access

The database is standard SQLite — use any tool:

```sql
-- Recent events
SELECT * FROM events ORDER BY occurred_at DESC LIMIT 10;

-- By type
SELECT * FROM events WHERE event_type = 'heart_rate';

-- Full-text search
SELECT e.* FROM events e
JOIN events_fts fts ON e.rowid = fts.rowid
WHERE events_fts MATCH 'keyword';

-- JSON queries
SELECT json_extract(data, '$.bpm') as bpm FROM events WHERE event_type = 'heart_rate';
```

## Design Principles

1. **Minimal** — Core stores data, plugins define meaning
2. **No dependencies** — Only Python standard library
3. **SQLite** — Recommended by Library of Congress for archival
4. **Event sourcing** — Never lose data, full history
5. **Flexible** — JSON data field for any structure

## Running Tests

```bash
cd core
python3 -m pytest tests/ -v
```

## License

MIT
