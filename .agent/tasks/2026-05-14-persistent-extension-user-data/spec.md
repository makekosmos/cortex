# Persistent extension user data

## Контекст

Extension installer MVP (см. `2026-05-14-extension-installer-mvp/`) делает install как «полный replace папки `<APPDATA>/Kosmos/extensions/<id>/`». Если extension писал что-либо внутрь своей папки (settings, window-state, кеш) — при reinstall эти файлы уничтожаются. Это блокер production rollout.

Решение — разделить «код extension'а» (replaceable) и «user data extension'а» (persistent) на два независимых path:

```
<APPDATA>/Kosmos/
├── extensions/<id>/         ← код (manifest, dist, icon) — replaceable, install трогает
└── extensions-data/<id>/    ← USER DATA — install НЕ трогает
    ├── settings.json
    ├── window-state.json
    └── ...
```

## Что НЕ входит

- Migration существующих файлов из `extensions/<id>/` в `extensions-data/<id>/` — текущий MVP не имеет таких файлов (gap по совпадению не срабатывает).
- UI manager для просмотра/чистки user-data — позже.
- Sandbox / per-extension storage quota — позже.

## Что входит

1. **Preload API `window.kepler.userData.*`** в `shell/electron/extension-preload.ts`:
   - `readJson<T>(name)` / `writeJson(name, value)` / `readFile(name)` / `writeFile(name, content)` / `path()`.
   - Path traversal protection: `name` matches `/^[\w][\w.-]*$/`.
   - `readJson` возвращает `null` если файла нет или JSON битый (warning в log).
   - Соответствующие IPC handlers в `shell/electron/extension-host.ts`.
   - Extension id определяется через `webContentsToExtensionId.get(e.sender.id)`.
2. **Window state automation** в `openExtension(id)`:
   - Read `extensions-data/<id>/window-state.json` перед `new BrowserWindow(...)`; sanitize bounds через `screen.getAllDisplays()`.
   - Save on `close`/`resized`/`moved`/`maximize`/`unmaximize` (debounced 500ms для resized/moved).
3. **`uninstall-extension.mjs --purge-data`** флаг:
   - Default: удаляет только code dir, user data сохраняется.
   - `--purge-data`: также удаляет `extensions-data/<id>/`.
4. **Docs updates** + `bun run docs:sync`.

## Acceptance criteria

- **AC1 (user data survives reinstall)**: install не трогает `extensions-data/<id>/`. PASS by code inspection.
- **AC2 (window state restored)**: shell пишет/читает `window-state.json`. PASS by inspection.
- **AC3 (uninstall preserves data by default)**: без флага — code dir удалён, data dir остался.
- **AC4 (--purge-data удаляет всё)**: с флагом — обе папки удалены.
- **AC5 (path traversal blocked)**: handler отклоняет `name` не подходящий под regex.
- **AC6 (typecheck + build)**: `bun run --cwd shell typecheck` clean; `bun run --cwd shell build:js` clean.

## Файлы

**Modify:**
- `shell/electron/extension-host.ts` — userData IPC handlers + window state в `openExtension`.
- `shell/electron/extension-preload.ts` — `kepler.userData.*` namespace.
- `shell/scripts/uninstall-extension.mjs` — `--purge-data` флаг.
- `docs-site/concepts/extension-installer.md`
- `docs-site/concepts/extension-host.md`
- `docs-site/apps/kepler-roadmap.md`

**Generated (через `docs:sync`):**
- `AGENTS.md`, `CLAUDE.md`, `docs-site/public/llms.txt`.

## План коммитов

1. IPC handlers + preload API surface (`extension-host.ts` + `extension-preload.ts`).
2. Window state automation в `openExtension`.
3. `uninstall-extension.mjs` — `--purge-data` flag.
4. Docs + `bun run docs:sync`.
