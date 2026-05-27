# Evidence — Akasha virtual scroll

| AC  | Status | Evidence                                                                                                                                                                                               |
| --- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| AC1 | PASS   | `Akasha` now stores `reader_blocks` and `reader_item_sizes`; `render()` no longer calls a method that clones all book blocks each frame.                                                               |
| AC2 | PASS   | Reader body uses `gpui_component::v_virtual_list` with visible `Range<usize>` rendering; visible rows render selectable `TextView` blocks.                                                             |
| AC3 | PASS   | TOC click calls `VirtualListScrollHandle::scroll_to_item(book.chapter_start_block(index), ScrollStrategy::Top)`; middle-button autoscroll updates the same virtual-list scroll handle on a 16ms timer. |
| AC4 | PASS   | `cargo check -p akasha`, `cargo test -p akasha`, `bun run --cwd shell typecheck`, and `bun run ark:guard:writes` passed after selectable text and middle-drag changes.                                 |

## Command Log

- `cargo fmt -p akasha` — PASS.
- `cargo check -p akasha` — PASS.
- `cargo test -p akasha` — PASS, 3 tests.
- `bun run --cwd shell typecheck` — PASS.
- `bun run ark:guard:writes` — PASS.
- Follow-up polish: selectable `TextView` rows, EPUB em-based typography tokens, `Aa` font picker, and middle-button autoscroll; same checks reran PASS.
- Follow-up UX/perf: reader virtual-list now renders an explicit vertical scrollbar, the open-book action is the first toolbar button, and native dev launch uses the release Akasha binary via `build:extensions`.
- `bun run docs:sync` — PASS.
- `bun run docs:check` — PASS.
- `bun run test:e2e -- tests/e2e/extensions-contract.spec.ts` — PASS, 5 tests.
- Follow-up responsive typography: virtual-list item sizes now track current reader width, paragraph/list gaps use em-based Apple Books rhythm, `epub:type="bridgehead"` and `cite` are preserved, and drag-selection shows a copy hint; `cargo test -p akasha`, `cargo check -p akasha`, `bun run --cwd shell typecheck`, and `bun run ark:guard:writes` reran PASS.
- Follow-up dev diagnostics: Kepler passes `--kosmos-dev-mode` to native extensions in dev sessions, and Akasha renders a compact FPS overlay from its render loop; `cargo test -p akasha`, `cargo check -p akasha`, `bun run --cwd shell typecheck`, and `bun run ark:guard:writes` reran PASS.
