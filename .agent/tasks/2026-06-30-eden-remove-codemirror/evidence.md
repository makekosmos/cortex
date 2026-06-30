# Evidence

Verified at 2026-06-30.

## AC1

PASS. `products/eden/src/editor-cm/` no longer exists.

Commands:

```powershell
Test-Path products/eden/src/editor-cm
rg --files products/eden/src products/eden/tests | rg "editor-cm|CmEditor|VimSettings|vimMotions|cmGate|CmConvert|cm-editor"
```

Result: `False`; second command returned no matches.

## AC2

PASS. Eden settings no longer expose Vim settings.

Evidence:

- `products/eden/src/views/EdenSettingsView.vue` has `SettingsTab = "general" | "export" | "trash"`.
- `products/eden/src/components/settings/SettingsPage.vue` renders only General, Export, and Trash settings.
- `products/eden/src/components/settings/VimSettings.vue` and `products/eden/tests/components/VimSettings.spec.ts` are deleted.

## AC3

PASS. Eden package and lockfile no longer contain CodeMirror, Vim, or Lezer direct dependencies.

Command:

```powershell
rg -n "@codemirror|@replit/codemirror-vim|@lezer|codemirror|CodeMirror|editor-cm|cmGate|CmEditor|vimMotions|VimSettings|\bvim\b|\bVim\b" products/eden/src products/eden/tests products/eden/package.json docs-site/apps/eden.md docs-site/apps/eden/editor.md bun.lock
```

Result: no matches.

## AC4

PASS. Shared content helpers moved to `products/eden/src/editor-content/content.ts`.

Command:

```powershell
rg -n "editor-content/content|editor-cm/content|cm-editor-test-helpers|ContentAdapter|editor-test-helpers" products/eden/src products/eden/tests
```

Result: source/tests import `editor-content/content`; no `editor-cm/content` or `cm-editor-test-helpers` remains.

## AC5

PASS. Verification commands passed.

Commands:

```powershell
bun run --cwd products/eden test
bun run products:build
bun run docs:check
bun run format:check
```

Results:

- Eden tests passed.
- Product build passed.
- Docs freshness check passed.
- Formatting check passed.
