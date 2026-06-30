# Eden TipTap/ProseMirror Spike

## Classification

FULL_LOOP.

## Goal

Create an isolated branch experiment that lets us run Eden with a TipTap/ProseMirror editor path, compare it against the current CodeMirror editor, and migrate current dev note bodies on demand.

## Scope

- Add an experimental TipTap editor path for Eden notes.
- Keep the current CodeMirror editor as the default/fallback.
- Add an explicit dev-only migration action that converts current Markdown body storage into TipTap JSON body storage through the normal Eden/ARK save path.
- Keep TipTap on a normal text caret; the fat cursor remains CodeMirror Vim-mode only.
- Do not carry Vim mode into the TipTap experiment.

## Out of Scope

- No automatic production data migration.
- No release/version bump.
- No attempt to fully replace CodeMirror.
- No raw SQL writes to ARK.
- No custom ProseMirror framework beyond the minimum wrapper needed for this spike.

## Acceptance Criteria

AC1. Eden can switch between the current CodeMirror editor and the experimental TipTap editor from local preferences without removing the CodeMirror path.

AC2. The TipTap editor loads existing Markdown-wrapper note bodies and saves edited content as TipTap JSON in `content_json`.

AC3. A dev-only migration action converts visible/current Eden entries from Markdown-wrapper bodies to TipTap JSON through the existing Eden save/API path, with a visible result count and no raw SQL writes.

AC4. The TipTap editor supports the minimum comparison surface: headings, paragraphs, bullet/ordered lists, task checkboxes, blockquote, code block, horizontal rule, slash command menu, autosave/draft updates, and title/type header behavior.

AC5. The TipTap editor uses a normal text caret, while Vim/fat-cursor behavior is not enabled in the TipTap path.

AC6. Targeted tests cover content conversion and migration safety, and the Eden Vue/unit checks run or failures are documented.

AC7. A visual/dev verification opens Eden with the experimental editor, captures a screenshot, and records what was and was not verified.
