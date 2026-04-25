# Task: Add SQLite FTS5 search for ARK objects

## Context

`db::search_objects` currently does simple in-memory scoring over object title/content/props. That is acceptable for small data sets but weak for ARK as a long-lived local-first object store. SQLite already ships FTS5 through the bundled `rusqlite` build, so generic ARK objects can have a durable full-text index without adding a separate search service.

## Acceptance Criteria

AC1. `init_schema` creates an FTS5 virtual table for searchable object text and rebuilds/repairs the index for existing objects.

AC2. `upsert_object` updates the FTS index atomically with the `objects` table.

AC3. `delete_object` removes deleted objects from the FTS index.

AC4. `search_objects` uses FTS5 `MATCH` for non-empty queries and returns stable `SearchResult` records for matching objects.

AC5. Search remains safe for user-entered punctuation and falls back gracefully when a query cannot be parsed by FTS5.

AC6. Tests prove indexing existing objects during schema init, updating/removing index entries on object writes, and punctuation-safe search.

AC7. Verification artifacts show Rust formatting, compile, full tests, and diff checks pass against the current codebase.
