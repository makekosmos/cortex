# 2026-06-04 — ARK upsert preserves object links

## Цель

Убрать опасный `INSERT OR REPLACE` из write-path generic object model в `ark_core::db`, потому что SQLite `REPLACE` удаляет старую строку перед вставкой и может каскадно удалить `object_links`.

## Scope

In scope:

- `crates/ark-core/rust/src/db.rs` helpers for `object_types`, `objects`, `object_links`.
- Regression tests around graph-preserving upsert semantics.
- Bug postmortem entry in `docs-site/agents/postmortems.md`.

Out of scope:

- Legacy tables (`todos`, `projects`, `areas`, `tags`, `headings`).
- Usage tables and `sync_kv`.
- Schema changes or sync wire-format changes.

## Acceptance Criteria

**AC1.** `upsert_object`, `upsert_object_type`, and `upsert_object_link` use SQLite `INSERT ... ON CONFLICT(id) DO UPDATE` semantics, not `INSERT OR REPLACE`, so existing rows are updated without delete+insert side effects.

**AC2.** Regression coverage proves that upserting an existing object after creating an `object_links` edge to or from it preserves that link.

**AC3.** Regression coverage proves that upserting an existing object type after objects of that type exist preserves those objects and their links.

**AC4.** Regression coverage proves that upserting an existing object link updates the link row in place without changing graph cardinality.

**AC5.** Required guards for this data-layer substantial fix pass: targeted Rust tests, `bun run ark:guard:writes`, `bun run ark:smoke`, and docs checks for the postmortem update.
