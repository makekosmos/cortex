# Evidence: Phase 5 — ts-rs codegen pilot

## Что сделано

### Rust ts-rs derives

`crates/ark-core/rust/Cargo.toml`:

```toml
ts-rs = { version = "10", optional = true }

[features]
ts-rs = ["dep:ts-rs"]
```

Production build (без `--features ts-rs`) не подтягивает ts-rs runtime —
только тесты которые экспортят bindings.

`crates/ark-core/rust/src/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct ArkObject { ... }
// + ObjectType, ObjectLink аналогично
```

Path `../../../../packages/...` — 4 dot-dots от source файла `types.rs`
(`crates/ark-core/rust/src/types.rs` → `crates/ark-core/rust/src/../../../../packages/...`).

`Value` поля маппятся через `#[ts(type = "unknown")]` (contentJson) или
`#[ts(type = "Record<string, unknown>")]` (propsJson).

### Generated bindings

`packages/ark/src/generated/`:

- `ArkObject.ts`
- `ObjectType.ts`
- `ObjectLink.ts`
- `index.ts` — re-export (с `.js` extensions для NodeNext module resolution)

Регенерация:

```
cargo test --features ts-rs --manifest-path crates/ark-core/rust/Cargo.toml --lib
```

### Public API

`packages/ark/src/index.ts`:

```ts
export type { ArkObject, ObjectType, ObjectLink } from "./generated/index.js";
```

Теперь `import { ArkObject } from "@kosmos/ark"` работает.

### Pre-commit guard

`lefthook.yml` → `pre-commit.commands.ts-rs-bindings-fresh`:

```yaml
glob: "crates/ark-core/rust/src/types.rs"
run: |
  cd crates/ark-core/rust && \
  cargo test --features ts-rs --quiet --lib export_bindings 2>&1 > /dev/null && \
  cd - && \
  git diff --exit-code packages/ark/src/generated/ || \
  (echo "ts-rs bindings out of date — re-run cargo test --features ts-rs and git add" && exit 1)
```

## AC verification

### AC1 — ts-rs в Cargo.toml

```toml
ts-rs = { version = "10", optional = true }
```

PASS. (Не dev-dep как было в спеке — optional dep в основном [dependencies]
с feature flag — чище, без double-build.)

### AC2 — Bindings генерируются

```
$ cargo test --features ts-rs --lib export_bindings
test types::export_bindings_arkobject ... ok
test types::export_bindings_objecttype ... ok
test types::export_bindings_objectlink ... ok
$ ls packages/ark/src/generated/
ArkObject.ts  ObjectLink.ts  ObjectType.ts  index.ts
```

PASS. (Location — `packages/ark/src/generated/` напрямую, не
`crates/ark-core/rust/bindings/` как в спеке — иначе TS rootDir
конфликт.)

### AC3 — generated/index.ts re-exports

`packages/ark/src/generated/index.ts` — 3 named type re-exports.

PASS.

### AC4 — public API re-export

`packages/ark/src/index.ts`:

```ts
export type { ArkObject, ObjectType, ObjectLink } from "./generated/index.js";
```

PASS.

### AC5 — packages/ark typecheck green

```
$ bun run --cwd packages/ark typecheck
$ tsc --noEmit
```

PASS.

### AC6 — shell typecheck green

```
$ bun run --cwd shell typecheck
$ tsc --noEmit
```

PASS.

### AC7 — Pre-commit guard работает

`bunx lefthook validate` → All good. Logic guard: change types.rs without
regen → cargo test перегенерит → git diff обнаружит unstaged delta → fail.

PASS (by construction).

### AC8 — e2e не сломан

```
$ bunx playwright test tests/e2e/eden.spec.ts
9 passed (55.8s)
```

PASS.

## Что вышло за spec scope

- Path: spec говорил `crates/ark-core/rust/bindings/`, реально
  `packages/ark/src/generated/`. Причина: TypeScript `rootDir`
  packages/ark = `src/`, любой импорт outside src/ ломает tsconfig.
  Решение: пусть ts-rs пишет прямо в packages/ark/src/generated/ —
  single source of truth остаётся в Rust, но физический файл живёт
  в TS workspace для resolver'а.

- ts-rs как `optional dependency` + feature flag, не `[dev-dependencies]`.
  Разница: dev-dep не доступен в `--features`, а нам нужен feature gate
  для conditional `#[derive(TS)]`. С optional dep + feature `ts-rs =
  ["dep:ts-rs"]` это работает.

## Out of scope (для следующих итераций)

- Расширить ts-rs на остальные wire-types: `TodoItem`, `Project`, `Area`,
  `Tag`, `UsageSession`, и т.д. (~10 типов).
- Полностью типизировать `request<Op>` в `packages/ark/src/ark-client.ts` —
  пока остаётся `request<T = unknown>`. Это самый big-win для catch'а
  protocol drift, но требует TypedMap всех operations (десятки).
- Migration ручных TS типов (`ArkObjectRecord` в ark-client.ts и т.п.) на
  generated. Сейчас они дублируют — duplicate можно удалить gradual.
