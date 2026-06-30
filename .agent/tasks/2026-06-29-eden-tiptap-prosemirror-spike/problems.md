# Problems

## AC7 initial issue resolved

- Symptom:
  - The first visual script mocked `window.api`, but `installKeplerApiShim()` replaces it, so no note mounted.
- Fix:
  - The visual script now mocks `window.kepler.ark.request/subscribe`, opens a mocked note, and captures `.tmp/visual/2026-06-29-eden-tiptap-spike/eden-tiptap-editor-1280x860.png`.
- Remaining rough edge:
  - The slash menu appears at the body start rather than at the caret coordinates. Good enough for spike visibility, not good enough for final UX.

## Browser/Vue suite is not a clean gate yet

- Symptom:
  - `rtk bun run --cwd products/eden test:vue` fails outside the TipTap path.
- Current failure modes seen in this turn:
  - `tests/components/VimSettings.spec.ts` still expects obsolete Vim/CM preference text and `setCmEditorEnabled`.
  - `tests/components/EdenSidebar.spec.ts` cannot find `object-type-note_obj` in the object picker.
- Impact on this spike:
  - Focused content/preferences/migration coverage is green, the Eden unit suite is green, and visual smoke for TipTap is green. The remaining failures should be cleaned as separate test debt.
