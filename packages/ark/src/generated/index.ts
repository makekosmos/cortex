// Re-export generated TS bindings из Rust ark-core types.
//
// Источник правды: `crates/ark-core/rust/src/types.rs` с `#[derive(TS)]`.
// Регенерация: `cargo test --features ts-rs --manifest-path
// crates/ark-core/rust/Cargo.toml --lib`. Затем git коммит изменений в
// этой директории (`packages/ark/src/generated/*.ts`).
//
// Phase 5 bug-detection — pilot для 3 core types. Полная типизация
// Request/Response variants — следующая итерация.

export type { ArkObject } from "./ArkObject.js";
export type { ObjectType } from "./ObjectType.js";
export type { ObjectLink } from "./ObjectLink.js";
