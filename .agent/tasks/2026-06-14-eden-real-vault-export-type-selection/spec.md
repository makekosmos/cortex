# Task Spec — Eden real Obsidian vault export with folder structure/assets and UI type selection

## Task ID / Path

- `2026-06-14-eden-real-vault-export-type-selection`
- `.agent/tasks/2026-06-14-eden-real-vault-export-type-selection/spec.md`

## Original task statement

> Task: Create/freeze a proof-loop spec for new FULL_LOOP task: Eden real Obsidian vault export with folder structure/assets and UI type selection. User request in Russian: "сделай real vault export и сделай возможность выбрать типы которые экспортируешь, чтобы например случайно не захватить ненужные задачи."
>
> Requirements to capture:
>
> - Export entire Eden/Obsidian-style vault preserving folder structure where available.
> - Include/copy/export images/assets and rewrite markdown references to relative paths when feasible.
> - Allow user to choose which object types are exported, so e.g. task objects can be excluded.
> - Preserve existing Markdown storage/import behavior; no direct SQL writes; no schema/destructive migrations unless explicitly justified (prefer none).
> - Minimal safe implementation with tests/docs/evidence.
>
> Return task id/path and frozen acceptance criteria. Do not implement production code.

## Context

Current Eden export is a flat Markdown export path in `products/eden/src/components/settings/GeneralSettings.vue` backed by `buildObsidianExportFiles()` in `products/eden/src/lib/obsidianVault.ts`.
It exports `.md` files only, without preserving folder hierarchy, and it does not currently provide a user-facing type filter for vault export.

Relevant repo guidance:

- Eden data must go through the existing ARK boundary; no direct SQL writes from app code.
- Markdown storage/import behavior must remain intact.
- Use minimal, narrow documentation only; do not broaden scope into unrelated refactors.

## Scope

In scope:

- Export a real Obsidian-compatible vault structure from Eden data.
- Preserve folder hierarchy where source metadata exists.
- Copy/export image and asset files alongside notes.
- Rewrite Markdown image/file references to relative export paths when feasible.
- Add UI for selecting which object types are included in export.
- Keep export implementation safe, incremental, and covered by tests.

Out of scope:

- Live file sync / watcher-based bidirectional vault mirroring.
- Obsidian plugin development.
- New data schema or destructive migrations.
- Changing canonical Eden storage away from Markdown-as-storage.
- Direct filesystem/database access outside existing safe APIs.

## Assumptions

- “Folder structure where available” means: use existing Eden folder metadata if present; do not invent a new folder model.
- “Object types” means Eden/ARK note/object types already known to the UI; task-like/system types must be explicitly opt-in or explicitly deselectable so they are not silently exported.
- If an asset target cannot be resolved safely, leave the original reference intact and surface a non-fatal warning rather than failing the whole export.

## Constraints

- No direct SQL writes.
- No schema migrations unless a later proof requires them and they are explicitly justified.
- Preserve existing Markdown storage/import behavior.
- Prefer additive, reversible changes.
- Keep implementation minimal and safe.
- Add tests for export path generation, asset copying/reference rewriting, and type filtering.

## Non-goals

- Perfect fidelity for every possible Obsidian feature.
- Round-trip sync parity with Obsidian.
- Full folder management UI redesign.
- Refactoring unrelated Eden editor or storage code.

## Acceptance Criteria

**AC1.** Eden can export an Obsidian-compatible vault rooted at a chosen output folder, producing Markdown files for selected entries and preserving folder hierarchy when the source data provides it.

**AC2.** Exported notes keep their Markdown body, and exported references to images/assets are copied into the vault and rewritten to relative paths when a safe local target can be determined.

**AC3.** The export UI lets the user choose which Eden object types are included in the vault export; excluded types are not written to the exported vault. Task-like objects are explicitly able to be excluded.

**AC4.** The type-selection UI is safe by default: it does not silently include hidden/system/task content without an explicit user choice, and the user can review the selected set before export starts.

**AC5.** Existing Markdown storage/import behavior remains unchanged: content continues to flow through the Markdown storage adapter and existing Eden save paths; export changes do not alter canonical `content_json` semantics.

**AC6.** The implementation respects the ARK write boundary: export reads may use existing safe app APIs, but no direct SQL writes or new destructive schema changes are introduced.

**AC7.** Relevant tests cover at least: folder path derivation, asset/reference rewriting, type exclusion, and a smoke export scenario with notes + assets. Docs are updated if the UI or export contract changes.

## Verification plan

- Run the focused Eden/unit tests for vault export helpers.
- Run the Eden extension build.
- Run the ARK write-boundary guard if any touched path is in a guarded app-service area.
- Manually smoke-check the exported vault structure and open it in Obsidian-compatible tooling if available.

## Notes

This is a FULL_LOOP task because it crosses UI, export logic, assets, and data-boundary concerns. The spec intentionally avoids implementing bidirectional sync or changing the storage model.
