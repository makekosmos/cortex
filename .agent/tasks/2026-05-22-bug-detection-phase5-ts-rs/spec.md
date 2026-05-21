# 2026-05-22 bug-detection-phase5-ts-rs

## Context

ARK wire-protocol типы поддерживаются **вручную** на двух сторонах:

- `crates/ark-core/rust/src/types.rs` (Rust struct'ы)
- `packages/ark/src/*.ts` (TS типы)

Если переименовать поле в Rust и забыть TS-сторону → silent runtime
mismatch. `request<T = unknown>` в `packages/ark/src/client.ts` принимает
любой generic, не валидируется.

Phase 5 из bug-detection roadmap. Цель — generated bindings из Rust в TS,
типизированный `request` контракт.

**Pilot scope**: первый шаг — 3 core types (ArkObject, ObjectType,
ObjectLink). Полная типизация всех Request/Response variants — следующая
итерация (отдельная задача), потому что у нас десятки операций и каждая
требует ручной аннотации.

## Scope

В задаче:

- Добавить `ts-rs` в `crates/ark-core/rust/Cargo.toml` как dev-dep с
  feature flag (build only when tests run).
- `#[derive(TS)]` + `#[ts(export, export_to = "...")]` для:
  - `ArkObject`,
  - `ObjectType`,
  - `ObjectLink`.
- `cargo test --workspace export_bindings` генерирует bindings в
  `crates/ark-core/rust/bindings/*.ts`.
- Re-export bindings в `packages/ark/src/generated/index.ts`.
- `packages/ark/src/index.ts` re-exports types from generated.
- Pre-commit guard в lefthook: если правил `.rs` в ark-core и не
  перегенерил bindings — fail (`git diff --exit-code bindings/`).

Не в задаче:

- Типизация всех Request/Response variants (десятки операций) — отдельная
  итерация.
- Сужение типа `request<Op>` в client.ts с typed map — отдельная задача
  после полной типизации.
- Полный refactor TS типов которые сейчас вручную дублируют ArkObject —
  можно сделать миграцию gradual в отдельной задаче.

## Acceptance Criteria

AC1. `crates/ark-core/rust/Cargo.toml` содержит `ts-rs` в `[dev-dependencies]`.

AC2. После `cargo test --workspace --lib -p ark-core --features ts-rs`
(или эквивалент) в `crates/ark-core/rust/bindings/` появляются:
`ArkObject.ts`, `ObjectType.ts`, `ObjectLink.ts`.

AC3. `packages/ark/src/generated/index.ts` re-exports эти типы.

AC4. `packages/ark/src/index.ts` re-export'ит generated types как public API.

AC5. `bun run --cwd packages/ark typecheck` зелёный — TS видит сгенерированные
типы корректно.

AC6. `bun run --cwd shell typecheck` зелёный — никаких регрессий в shell где
используется `@kosmos/ark`.

AC7. Pre-commit guard работает: правка `crates/ark-core/rust/src/types.rs`
без regen → `git diff --exit-code bindings/` fails.

AC8. `bunx playwright test tests/e2e/eden.spec.ts` зелёный — рантайм не сломан.

## Out of scope decisions

- `Value` (serde_json::Value) маппится в TS как `unknown` или `Record<string,
unknown>` через `#[ts(type = "unknown")]`. Не пытаемся типизировать content_json
  (это TipTap JSON, у которого свой type set).
- `Option<T>` → `T | null` (ts-rs default).
- camelCase serialization в Rust → camelCase в TS (тоже default).
