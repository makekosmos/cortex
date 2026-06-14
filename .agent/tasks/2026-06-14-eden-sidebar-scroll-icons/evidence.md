# Evidence — 2026-06-14-eden-sidebar-scroll-icons

Mode: EVIDENCE
Overall: UNKNOWN (AC9 not visually verified; unrelated test failures remain)

## Commands run

- `bun run --cwd products/eden test:unit` → failed; unrelated existing failures in `products/eden/tests/preferences.test.ts` and `products/eden/tests/vimMotions.test.ts`.
- `bun run products:build` → passed.
- `bun run --cwd products/eden test:vue` → failed; unrelated existing failures in `tests/components/VimSettings.spec.ts` plus a stale sidebar spec expectation for `settings-nav-storage`.
- `bun test products/eden/tests/components/EdenSidebar.spec.ts` → failed because `vitest/browser` requires browser mode.
- `bun run --cwd products/eden test:vue -- tests/components/EdenSidebar.spec.ts` → failed because the CLI treated the positional arg as a browser filter.

Raw logs:

- `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/artifacts/test-unit.log`
- `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/artifacts/products-build.log`
- `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/artifacts/test-vue.log`
- `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/artifacts/eden-sidebar-spec.log`
- `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/artifacts/eden-sidebar-vue.log`

## Acceptance criteria

| AC  | Status  | Proof                                                                                                                                                                                                                                                                                                                                               |
| --- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1 | PASS    | `products/eden/src/components/sidebar/EdenSidebar.vue:17-77` now puts the whole sidebar body in one `overflow-y-auto kosmos-scroll` container, and the nested `Недавние` / `Объекты` group bodies at `:31-63` are plain flex containers with no inner overflow.                                                                                     |
| AC2 | UNKNOWN | The single-scroll structure and `mt-auto` footer are in place (`:17-77`), but I did not run a long-data overflow/manual reachability check.                                                                                                                                                                                                         |
| AC3 | PASS    | `recentItems` still derives from `currentEntry` + `recentEntries` (`:355-361`) and `buildEntryItem()` resolves the note type by `entry.type_id` (`:221-233`), so typed objects remain eligible in recent.                                                                                                                                           |
| AC4 | PASS    | `sortedNoteTypes` sorts all `props.noteTypes` deterministically (`:193-205`), and the object-types screen renders both system and custom groups from that full set (`:364-381`).                                                                                                                                                                    |
| AC5 | PASS    | `phosphorSidebarIcon()` forces `weight: "duotone"` and icon color (`:207-218`), and type rows use `noteType.color ?? TYPE_ICON_COLOR_FALLBACK` (`:235-245`).                                                                                                                                                                                        |
| AC6 | PASS    | Entry rows use `ENTRY_ICON_COLOR = "var(--muted-foreground)"` (`:172-173`, `:221-233`), so recent/object page rows stay neutral while type/list rows keep their colored treatment.                                                                                                                                                                  |
| AC7 | PASS    | The component still emits the same navigation actions: create note/search/open settings/open object types/create object type/back/open entry (`:248-315`, `:319-416`, `:221-245`). Browser test output in `artifacts/test-vue.log` shows those sidebar controls render and are clickable before the stale `settings-nav-storage` expectation fails. |
| AC8 | PASS    | `git diff -- products/eden/src/components/sidebar/EdenSidebar.vue` shows the only production change is this Eden sidebar component; no ARK/data/sync/schema files were touched. `products:build` passed.                                                                                                                                            |
| AC9 | UNKNOWN | No explicit visual overflow test was run with a forced-overflow dataset; there is no proof here of “one scrollbar / no inner scrollbars” beyond the structure in AC1.                                                                                                                                                                               |

## Known failing tests

- `products/eden/tests/preferences.test.ts` in `test-unit.log`: 2 existing failures (`spellcheckEnabled` / state isolation).
- `tests/components/VimSettings.spec.ts` in `test-vue.log`: 2 existing failures about the CM6 warning copy.
- `products/eden/tests/components/EdenSidebar.spec.ts` in `test-vue.log`: stale expectation for `settings-nav-storage` (not part of the current spec).
