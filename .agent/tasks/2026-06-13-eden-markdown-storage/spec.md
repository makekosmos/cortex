# Task Spec — Eden Markdown-as-storage migration

## Source of truth

- User decision: Eden should move to Markdown-as-storage now; Eden has not been used heavily enough to justify preserving ProseMirror JSON as canonical body storage.
- Current state after `81c564b`: `App.vue` always routes current entries to CodeMirror (`CmEditor.vue`); TipTap UI is no longer in the runtime editor path, but TipTap is still used headlessly by `editor-cm/mdConvert.ts`.
- Relevant docs: `docs-site/apps/eden/editor.md`, `docs-site/apps/eden/data.md`, `docs-site/concepts/write-boundary.md`.

## Decision

Canonical Eden body storage in ARK `content_json` becomes Markdown:

```json
{
  "type": "markdown",
  "version": 1,
  "text": "Markdown body..."
}
```

ARK remains the source of truth. File/Obsidian integration is a later sync/mirror layer over ARK, not the foundation of this migration.

## Migration safety stance

Legacy Eden data is considered disposable/test data. Therefore this migration does **not** aim for lossless ProseMirror → Markdown conversion and does **not** need one-shot migration. The safe path is:

- new writes use Markdown storage immediately;
- old/invalid content is read tolerantly;
- no crashes on strange `content_json`;
- old objects naturally become Markdown only if opened/edited/saved through the normal ARK save path;
- legacy read tolerance is permanent unless legacy objects are explicitly deleted through the normal ARK delete path.

## Non-negotiable migration safety rules

1. **Markdown reads are idempotent.**
   - `isMarkdownContent(cj)` is strict: `cj?.type === "markdown"`.
   - `readEntryMarkdown` on `{ type: "markdown", version: 1, text }` returns `text` unchanged.
   - Re-opening/re-saving must not double-wrap or normalize content unnecessarily.

2. **Legacy reads are best-effort only.**
   - Legacy ProseMirror JSON is converted by recursive text/media extraction, not a fidelity-preserving Markdown serializer.
   - Unknown node types do not block the editor; extract nested text if present, otherwise ignore/empty fallback.
   - Image/media-like legacy nodes must preserve their source when available by emitting Markdown image syntax `![](src)` (check likely attrs such as `src`, `url`, `href`, `fileUrl`). Generic text extraction alone would silently erase image-only objects on first save.
   - The editor must never crash on invalid/empty/unknown `content_json`.

## Acceptance criteria

- AC1: New/edited Eden entries save `content_json` as `{ type: "markdown", version: 1, text: string }`.
- AC2: Existing markdown objects load with exact `.text` body, without conversion or mutation.
- AC3: Legacy ProseMirror JSON loads through standalone best-effort `legacyProseMirrorToText` without TipTap.
- AC4: Legacy reader has lightweight smoke coverage: representative legacy object(s) produce some sensible text/media markdown and never crash. Node-by-node formatting fidelity is not required; image-only legacy content must produce `![](src)` when a source attr exists.
- AC5: Invalid/empty content reads as empty Markdown, with no crash.
- AC6: In commit 1, Markdown import/export call the new Markdown content adapter directly and no longer call PM JSON conversion. The dead `mdConvert.ts` file is removed in commit 2.
- AC7: TipTap UI files and TipTap dependencies are removed only after build/tests prove no live imports remain.
- AC8: All writes still go through existing Eden/ARK save paths; no direct SQL writes.

## Planned commits

### Commit 1 — Markdown storage adapter + CM write path

Files likely touched:

- Add `products/eden/src/editor-cm/content.ts`:
  - `MARKDOWN_CONTENT_VERSION`
  - `MarkdownContent` type
  - `isMarkdownContent(value)`
  - `readEntryMarkdown(value | string)`
  - `writeEntryMarkdown(md)`
  - `legacyProseMirrorToText(value)`
- Add lightweight tests for markdown idempotency and legacy/invalid no-crash behavior.
- Update `products/eden/src/editor-cm/CmEditor.vue`:
  - load: parse `entry.content_json`, then `readEntryMarkdown(...)`.
  - save: `JSON.stringify(writeEntryMarkdown(md))`.
  - remove live `createMdConverter` / `mdConvert` use.
- Update `products/eden/src/components/settings/GeneralSettings.vue`:
  - export body via `readEntryMarkdown(...)`.
  - import body via `writeEntryMarkdown(parsed.bodyMarkdown)`.
- Update new-entry/default content creation:
  - `products/eden/src/store/eden.ts`
  - `products/eden/src/lib/kepler-api-shim.ts`
  - `products/eden/src/lib/obsidianVault.ts` (already creates image object drafts with legacy `{ type: "doc" }`; update or consciously scope if image bodies stay non-markdown).
- Update docs: `docs-site/apps/eden/editor.md` and maybe `docs-site/apps/eden/data.md`.

Verification for commit 1:

- Unit tests for content adapter, including load → write without edits preserves markdown `.text` byte-for-byte.
- Eden extension build.
- Relevant docs check.
- Manual/automated smoke: create note, type markdown, save, reopen, verify `content_json.type === "markdown"` and text unchanged.

### Commit 2 — Remove TipTap legacy UI and dependencies

Files likely removed if build confirms no imports:

- `products/eden/src/Editor.vue`
- `products/eden/src/Editor.css`
- `products/eden/src/BlockSelectionDecoration.ts`
- `products/eden/src/InlineCaret.ts`
- `products/eden/src/SlashCommand.ts`
- `products/eden/src/SlashCommandList.vue`
- `products/eden/src/TaskRef.ts`
- `products/eden/src/TrailingParagraph.ts`
- `products/eden/src/Wikilink.ts`
- TipTap-only NodeViews/components if unused by CM path.
- `products/eden/src/editor-cm/mdConvert.ts`
- TipTap/PM/lowlight deps from `products/eden/package.json` and lockfile.

Verification for commit 2:

- `rtk grep` no live `@tiptap` imports under Eden runtime.
- Eden extension build.
- Unit tests still pass.

### Commit 3+ — ARK-authoritative file/Obsidian bridge

Later, after Markdown storage is stable:

1. Export-only mirror to `.md` files with frontmatter.
2. File watcher/import back into ARK with hash/mtime tracking.
3. Conflict policy: last-writer-wins + `.conflict.md` fallback.
4. New file in Obsidian → mint ARK id → write `kosmos_id` back to frontmatter.
5. Delete/rename policy.
6. Optional Obsidian plugin for metadata/sync-status/ARK commands.

## Obsidian/file bridge target model

Source of truth remains ARK. Files are a sync projection:

```markdown
---
kosmos_id: "..."
type_id: "note_obj"
updated_at: 1718000000000
---

Markdown body...
```

Obsidian compatibility considerations:

- Body is plain Markdown.
- Wikilinks prefer `[[Title]]` so Obsidian graph/backlinks work natively.
- Stable ARK identity lives in frontmatter `kosmos_id`.

## CM editor forward-design notes

These are not blockers for commit 1 because legacy data is disposable and legacy reading is best-effort only:

- New task/checklist syntax should use Markdown/GFM `- [ ]` / `- [x]`.
- New wikilinks should prefer Obsidian-native `[[Title]]` so future file mirrors get graph/backlinks for free.
- New task-reference syntax can be decided when CM task-linking is implemented (`[[task:<id>]]` vs `[Title](kosmos://task/<id>)` or another stable form).

## Open questions before file/Obsidian bridge

- Should file mirror include all markdown-bodied objects or only specific note types?
- Does title follow file rename, or is title only ARK metadata?
- Delete policy for files: soft-delete object vs ignore vs move to trash.
