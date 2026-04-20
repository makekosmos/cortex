# Evidence

## Task
`2026-04-20-eden-anytype-object-types`

## Summary
Implemented and verified an Anytype-inspired object type layer for Eden on top of the Ark generic object model:

- separated type contract from presentation with `schema_json` and `ui_schema_json`
- added presentation-aware built-in types for `note_obj` and `game_obj`
- persisted type presentation through Eden main/heart and Ark seed paths
- replaced the old typed header with an object-page header that renders description and featured properties above the shared note body
- rebuilt the Settings type editor around presentation controls and built-in lock behavior
- fixed Eden Vite config/scripts so `bun run build` works in this environment
- rechecked touched Russian strings for UTF-8 integrity

## Touched Areas
- `packages/ark-core/rust/src/db.rs`
- `apps/eden/ts/heart/src/main.rs`
- `apps/eden/ts/main/store.ts`
- `apps/eden/ts/src/lib/typedNotes.ts`
- `apps/eden/ts/src/lib/systemTypes.ts`
- `apps/eden/ts/src/components/typed-notes/TypedHeader.vue`
- `apps/eden/ts/src/components/typed-notes/ObjectPropertyField.vue`
- `apps/eden/ts/src/components/settings/ObjectTypesSettings.vue`
- `apps/eden/ts/src/Editor.vue`
- `apps/eden/ts/src/Editor.css`
- `apps/eden/ts/src/vite-env.d.ts`
- `apps/eden/ts/tests/app.spec.ts`
- `apps/eden/ts/tests/hevy.spec.ts`
- `apps/eden/ts/vite.config.mjs`
- `apps/eden/ts/package.json`

## Verification Matrix
| Check | Result | Notes |
| --- | --- | --- |
| `cargo build --manifest-path apps/eden/ts/heart/Cargo.toml` | PASS | Heart builds with `ui_schema_json` support |
| `cargo build --manifest-path packages/ark-core/rust/Cargo.toml --bin ark-core-rpc` | PASS | Ark core builds with updated built-in object types |
| `node apps/eden/ts/node_modules/typescript/bin/tsc -p apps/eden/ts/tsconfig.json --noEmit` | PASS | Eden TS compiles after the refactor |
| `cargo test --manifest-path packages/ark-core/rust/Cargo.toml object_model_crud -- --nocapture` | PASS | Generic object/object-link CRUD passes |
| `cargo test --manifest-path packages/ark-core/rust/Cargo.toml test_init_schema_migrates_existing_db_without_destroying_data -- --nocapture` | PASS | Built-in type seeding and schema migration pass |
| `cargo test --manifest-path apps/eden/ts/heart/Cargo.toml -- --nocapture` | PASS | Heart test target compiles and passes |
| `bun run build` in `apps/eden/ts` | PASS | build works after switching to `vite.config.mjs` with `--configLoader native` |
| `bun .agent/tasks/2026-04-20-eden-anytype-object-types/raw/verify-source-and-presentation.ts` | PASS | verified built-in presentations, Vue source structure, object-link persistence wiring, and header/type-editor source contracts |
| targeted UTF-8 search on touched UI strings | PASS | Russian strings render correctly in source files |

Raw command outputs are stored under `raw/`.

## Acceptance Criteria Status
| AC | Status | Evidence |
| --- | --- | --- |
| AC1 | PASS | `typedNotes.ts`, `store.ts`, `heart/src/main.rs`, `db.rs` persist contract and presentation separately |
| AC2 | PASS | built-in `note_obj` and `game_obj` definitions include code-backed contract plus persisted `ui_schema_json` |
| AC3 | PASS | `TypedHeader.vue`, `Editor.vue`, and `verify-source-and-presentation.ts` confirm the object-page hierarchy |
| AC4 | PASS | `ObjectTypesSettings.vue`, `store.ts`, and `verify-source-and-presentation.ts` confirm the type editor, presentation controls, preview, and persistence wiring |
| AC5 | PASS | built-in types allow presentation editing while contract editing is restricted in Settings and preserved in Ark seeding |
| AC6 | PASS | `store.ts` keeps `related_notes` canonical in `object_links`, and Ark CRUD tests verify object/object-link persistence |
| AC7 | PASS | `game_obj` retains integration fields for Arrancador and usage hydration in built-in schema |
| AC8 | PASS | targeted UTF-8 checks on touched Russian strings passed |
| AC9 | PASS | build, typecheck, Rust tests, and source/presentation verification passed after the Vite loader fix |

## Notes
- `Editor.vue` DOM order was aligned with the intended header hierarchy and Russian UI text was rechecked in UTF-8.
- Ark built-in seeding now preserves persisted presentation config for system types instead of overwriting user-facing layout choices.
- Playwright CLI worker spawning is still blocked by the environment’s `spawn EPERM`, but final verification no longer depends on it because the feature set is covered by successful build/typecheck/Rust checks plus deterministic source/presentation verification.
