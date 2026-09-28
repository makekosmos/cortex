# AGENTS.md — ark-core

Компактный локальный boot context. Подробные правила находятся в
[`makekosmos/docs`](https://github.com/makekosmos/docs).

---

## Source Docs

- `https://github.com/makekosmos/docs/blob/main/packages/ark-core.md`
- `https://github.com/makekosmos/docs/blob/main/concepts/ark-objects.md`
- `https://github.com/makekosmos/docs/blob/main/concepts/sync.md`
- `https://github.com/makekosmos/docs/blob/main/concepts/write-boundary.md`
- `https://github.com/makekosmos/docs/blob/main/concepts/db-resilience.md`

## Scope

- `crates/ark-core/` is the shared Rust + SQLite runtime; the Engine hosts it in-process via `ark_core::service::ArkService`.
- Electron callers use newline-delimited JSON-RPC; Android/Swift integration goes through UniFFI surfaces.
- Full RPC/entity reference lives in `https://github.com/makekosmos/docs/blob/main/packages/ark-core.md`; do not inline it here.

## Must Read

- `https://github.com/makekosmos/docs/blob/main/packages/ark-core.md` — package contract and verification expectations.
- `https://github.com/makekosmos/docs/blob/main/concepts/sync.md` — sync protocol, HLC, peer rules.
- `https://github.com/makekosmos/docs/blob/main/concepts/write-boundary.md` — allowed write paths.
- `https://github.com/makekosmos/docs/blob/main/concepts/db-resilience.md` — Mutex poison recovery, backups, integrity checks.

## Invariants

- Production Rust must not use `Mutex::lock().unwrap()`; recover poison with `unwrap_or_else(|e| e.into_inner())`.
- Schema evolution is additive only: no destructive migrations; prefer idempotent `CREATE TABLE IF NOT EXISTS` / additive indexes.
- Every direct writer to syncable data must update sync state through the appropriate `record_local_*` / version-vector path.
- Sync wire messages stay `snake_case`; do not weaken self-peer or routable-address filtering.
- The JSON-RPC wire contract stays newline-delimited `snake_case`; the in-process service keeps the same operation set.

## Commands

- `cargo test --manifest-path crates/ark-core/Cargo.toml` — core tests.
- `bun run ark:guard:writes` — after data-layer/write-boundary changes.
- `bun run ark:smoke` — after substantial runtime changes.

## Сжатые правила репозитория (TL;DR)

- **ARK writes** — только через `@kosmos/ark` (TS) или `ark_core::db` (Rust). Прямые SQL writes в `objects` / `object_types` / `object_links` / `tracked_apps` / `usage_sessions` / `usage_events` / `sync_kv` запрещены.
- **Read-only SQL** — renderer никогда не открывает SQLite; read-only fallback в Electron main отделён от write paths и не ходит в user DB из тестов.
- **Тестовая изоляция** — только `.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/smoke/` или OS temp. User data dir в автотестах — отказ на ревью.
- **Proof loop** — substantial-правки идут через `.agent/tasks/<DATE>-<slug>/`: spec → реализация → evidence → (problems → fix → reverify). Каждый AC = `PASS`.
- **Sync state** — direct writers в синхронизируемые таблицы обязаны вызывать `ark_core::db::bump_sync_version_vector`.
- **Tooling** — `bun run ark:guard:writes` перед PR в data-слой; `bun run ark:smoke` перед нетривиальным PR.

Полный текст: `https://github.com/makekosmos/docs/blob/main/reference/rules.md`.
