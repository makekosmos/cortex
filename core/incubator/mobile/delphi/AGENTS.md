# AGENTS.md — Delphi Android

Компактный локальный boot context. Подробные правила находятся в
[`makekosmos/docs`](https://github.com/makekosmos/docs).

---

## Source Docs

- `https://github.com/makekosmos/docs/blob/main/apps/index.md#android-kotlin`
- `https://github.com/makekosmos/docs/blob/main/apps/ark-service.md`
- `https://github.com/makekosmos/docs/blob/main/apps/delphi.md`

## Scope

- This file applies only to `incubator/mobile/delphi/` Android code.
- Desktop Delphi lives in `products/delphi/`; do not apply desktop ARK or Vue assumptions here unless the task explicitly says so.
- Android Delphi is package `com.kazui.delphi` and consumes data from the separate `incubator/mobile/ark-service` APK.

## Must Read

- `https://github.com/makekosmos/docs/blob/main/apps/index.md#android-kotlin` — current Android stack snapshot.
- `https://github.com/makekosmos/docs/blob/main/apps/ark-service.md` — ContentProvider / Room ownership model.
- `https://github.com/makekosmos/docs/blob/main/agents/checklists.md` — Android checklist before handoff.

## Invariants

- Android Delphi does not own the Room DB directly; data access goes through `com.kosmos.ark.data` ContentProvider from `incubator/mobile/ark-service`.
- The Android stack is currently isolated from desktop ARK: no desktop `ark.db`, no `ark-core-rpc`, no working desktop<->Android sync assumption.
- If `ark-service` is absent, Delphi must keep the install-required UX instead of crashing or silently writing elsewhere.
- Changes to Android schema/provider contracts must be coordinated with `incubator/mobile/ark-service/`.

## Commands

- `cd incubator/mobile/delphi; .\gradlew build` — Android Delphi build.
- `cd incubator/mobile/ark-service; .\gradlew build` — provider build when provider contract changes.

## Сжатые правила репозитория (TL;DR)

- **ARK writes** — только через `@kosmos/ark` (TS) или `ark_core::db` (Rust). Прямые SQL writes в `objects` / `object_types` / `object_links` / `tracked_apps` / `usage_sessions` / `usage_events` / `sync_kv` запрещены.
- **Read-only SQL** — renderer никогда не открывает SQLite; read-only fallback в Electron main отделён от write paths и не ходит в user DB из тестов.
- **Тестовая изоляция** — только `.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/smoke/` или OS temp. User data dir в автотестах — отказ на ревью.
- **Proof loop** — substantial-правки идут через `.agent/tasks/<DATE>-<slug>/`: spec → реализация → evidence → (problems → fix → reverify). Каждый AC = `PASS`.
- **Sync state** — direct writers в синхронизируемые таблицы обязаны вызывать `ark_core::db::bump_sync_version_vector`.
- **Tooling** — `bun run ark:guard:writes` перед PR в data-слой; `bun run ark:smoke` перед нетривиальным PR.

Полный текст: `https://github.com/makekosmos/docs/blob/main/reference/rules.md`.
