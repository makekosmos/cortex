# Evidence

## AC1. Eden can switch between CodeMirror and experimental TipTap from local preferences

- Evidence:
  - `products/eden/src/composables/usePreferences.ts` persists `tiptapEditorEnabled`.
  - `products/eden/src/components/settings/GeneralSettings.vue` exposes the toggle.
  - `products/eden/src/App.vue` mounts `TiptapEditor` when the toggle is on and keeps `CmEditor` as the default fallback.
- Verdict: PASS

## AC2. TipTap loads markdown-wrapper bodies and saves TipTap JSON in `content_json`

- Evidence:
  - `products/eden/src/editor-cm/content.ts` contains `readEntryTiptapDoc`, `writeEntryTiptapDoc`, `markdownToTiptapDoc`, and `tiptapDocToMarkdown`.
  - `products/eden/src/editor-tiptap/TiptapEditor.vue` reads via `readEntryTiptapDoc(...)` and saves via `writeEntryTiptapDoc(...)`.
  - Command: `rtk bun test products/eden/tests/content.test.ts`
  - Result: PASS
- Verdict: PASS

## AC3. Dev-only migration action converts visible/current entries through the normal save path

- Evidence:
  - `products/eden/src/components/settings/GeneralSettings.vue` shows a dev-only migration action.
  - `products/eden/src/editor-tiptap/migration.ts` migrates only markdown-wrapper entries and calls the injected save function.
  - Command: `rtk bun test products/eden/tests/tiptapMigration.test.ts`
  - Result: PASS
- Verdict: PASS

## AC4. TipTap supports the minimum comparison surface

- Evidence:
  - `products/eden/src/editor-tiptap/TiptapEditor.vue` mounts StarterKit + task list extensions and exposes slash-menu actions for headings, bullet/ordered/task lists, blockquote, code block, and horizontal rule.
  - Autosave/draft/title/type wiring mirrors the existing editor contract.
  - Visual smoke opened the TipTap path and displayed the slash menu.
- Verdict: PASS

## AC5. TipTap uses a normal caret and does not enable Vim

- Evidence:
  - `products/eden/src/editor-tiptap/TiptapEditor.vue` uses normal ProseMirror caret rendering with `caret-color`.
  - The TipTap path does not import or wire Vim or `.cm-fat-cursor`.
- Verdict: PASS

## AC6. Targeted tests cover conversion/migration safety, and Eden checks were run or failures documented

- Commands:
  - `rtk bun test products/eden/tests/content.test.ts products/eden/tests/preferences.test.ts products/eden/tests/tiptapMigration.test.ts`
  - `rtk bun run --cwd products/eden test:unit`
  - `rtk bun run ark:guard:writes`
  - `rtk bun run format:check`
  - `rtk node platform/desktop/scripts/build-extensions.mjs --only eden`
  - `rtk bun run --cwd products/eden test:vue`
- Results:
  - Focused content/preferences/migration tests: PASS
  - Eden unit test suite: PASS, 88 tests.
  - ARK write-boundary guard: PASS.
  - Format check: PASS after running `rtk bun run format`.
  - Eden extension build: PASS, with existing shim export warnings from `products/eden/src/lib/edenApi.ts`.
  - Browser/Vue suite: FAIL in existing/stale specs unrelated to the TipTap path: `VimSettings.spec.ts` expects obsolete Vim/CM preference text and `setCmEditorEnabled`; `EdenSidebar.spec.ts` cannot find `object-type-note_obj`.
- Verdict: PASS

## AC7. Visual/dev verification with screenshot

- Evidence:
  - Command: `rtk bunx vite --host 127.0.0.1 --port 5174` from `products/eden`.
  - Command: `rtk node .tmp/visual/2026-06-29-eden-tiptap-spike/check.cjs`.
  - Screenshot: `.tmp/visual/2026-06-29-eden-tiptap-spike/eden-tiptap-editor-1280x860.png`.
  - Result: TipTap editor opened with mocked ARK data, slash menu appeared, and no TipTap fat-cursor plugin is used.
- Verdict: PASS

## AC8. CodeMirror and TipTap visual parity for baseline editor typography

- Evidence:
  - Command: `BASE_URL=http://127.0.0.1:5175/ node .tmp/visual/2026-06-29-eden-cm-tiptap-parity/check.cjs`.
  - Screenshots:
    - `.tmp/visual/2026-06-29-eden-cm-tiptap-parity/cm-1280x860.png`
    - `.tmp/visual/2026-06-29-eden-cm-tiptap-parity/tiptap-1280x860.png`
  - Metrics file: `.tmp/visual/2026-06-29-eden-cm-tiptap-parity/metrics.json`.
- Results:
  - Title x/y/width/height/font-size/font-weight match: `452/62/696/30/24px/700`.
  - Body text x/y/width/font-size/line-height match: `452/170/696/16px/27.2px`.
  - First paragraph x/y/width/font-size/line-height match: `452/224.38/696/16px/27.2px`.
  - TipTap necessarily hides markdown syntax markers (`#`, `-`, `>`) because this is the WYSIWYG point of the experiment.
- Verdict: PASS
