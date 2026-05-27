# Evidence — Akasha continuous reader

## Acceptance Criteria

| AC  | Status | Evidence                                                                                                                                                                                  |
| --- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1 | PASS   | `apps/akasha/src/epub.rs` now emits `ReaderBlock { kind, spans }`; `cargo test -p akasha` includes `extracts_spine_blocks_in_order` and `preserves_reader_structure_and_inline_emphasis`. |
| AC2 | PASS   | `apps/akasha/src/main.rs` flattens all chapter blocks through `reader_blocks()` and renders them into one reader scroll surface.                                                          |
| AC3 | PASS   | Permanent left sidebar removed; top `☰` button toggles a max-height `overflow_y_scrollbar()` chapters panel; clicking a chapter calls `reader_scroll.scroll_to_top_of_item(...)`.        |
| AC4 | PASS   | `render_reader_block` styles headings, paragraphs, list items, blockquotes; `StyledText::with_highlights` applies bold/italic spans.                                                      |
| AC5 | PASS   | Reader uses a persistent `ScrollHandle` with `.overflow_y_scroll().track_scroll(...)`; chapter navigation updates scroll target instead of swapping selected chapter content.             |
| AC6 | PASS   | `cargo test -p akasha`, `bun run test:e2e -- tests/e2e/extensions-contract.spec.ts`, and `bun run ark:smoke` passed.                                                                      |

## Command Log

- Regression before fix: `cargo test -p akasha preserves_reader_structure_and_inline_emphasis` failed because parser returned `["One", "Scene", "Plain bold and italic ."]` instead of preserving structure/emphasis.
- `cargo fmt -p akasha` — PASS.
- `cargo check -p akasha` — PASS.
- `cargo test -p akasha` — PASS, 3 tests.
- `bun run --cwd shell typecheck` — PASS.
- `bun run docs:sync` — PASS.
- `bun run ark:guard:writes` — PASS.
- `bun run test:e2e -- tests/e2e/extensions-contract.spec.ts` — PASS, 5 tests.
- `bun run docs:check` — PASS.
- `bun run --cwd shell build:extensions` — PASS.
- `bun run docs:build` — PASS.
- `bun run ark:smoke` — first retry exposed a live repo Electron holding `target/debug/ark-core-rpc.exe`; after stopping that dev Electron process, rerun PASS.
