# Extension installer MVP

## Контекст

Сейчас extensions (Dashboard, Horologion, Delphi, Arrancador) уезжают в Kepler installer через `extraResources` в `apps/kepler-shell/package.json`. Любое обновление логики extension'а требует rebuild + переустановки всего shell — это блокирует итерацию по конкретным апкам.

Цель: позволить ставить/обновлять extensions независимо от Kepler shell. Built-ins остаются bundled как fallback; user-installed копия в `%APPDATA%\Kosmos\extensions\<id>\` перекрывает bundled. Удалил user-папку → откатилось на bundled.

## Что НЕ входит в MVP

- Auto-update / version comparison из remote URL — в roadmap.
- UI в Kepler settings для management installed extensions — позже.
- `.kext` пакетный формат + file association в Windows — позже.
- Code signing / verification — позже.

## Что входит

1. **Resolution chain** в `extension-host.ts`:
   - Dev (`apps/kepler-shell/extensions/`) — если папка существует, наивысший приоритет (работа в repo).
   - User-installed (`%APPDATA%\Kosmos\extensions\<id>\`) — основной канал для prod.
   - Bundled (`<resourcesPath>/extensions/<id>\`) — fallback в packaged.
2. **CLI installer**: `bun run --cwd apps/kepler-shell ext:install <path-to-extension-dir>`.
   - Читает `manifest.json` из source dir → берёт `id`.
   - Копирует source → `%APPDATA%\Kosmos\extensions\<id>\` (atomic: temp + rename, чтобы не оставить полу-перенесённое состояние при ошибке).
   - Если target уже существует — удаляет перед перезаписью.
3. **CLI uninstaller**: `bun run --cwd apps/kepler-shell ext:uninstall <id>`.
   - Удаляет `%APPDATA%\Kosmos\extensions\<id>\`.
   - После этого launcher автоматически использует bundled версию (если есть).

## Acceptance criteria

- **AC1 (resolution order, dev)**: запущенный из repo Kepler shell видит extension'ы из `apps/kepler-shell/extensions/` (текущее поведение не сломано).
- **AC2 (resolution order, user override)**: при наличии `%APPDATA%\Kosmos\extensions\<id>\<manifest>.json` в packaged shell, openExtension читает именно эту копию, не bundled.
- **AC3 (resolution order, bundled fallback)**: при отсутствии user-installed копии, packaged shell читает bundled.
- **AC4 (CLI install)**: `bun run --cwd apps/kepler-shell ext:install ./some/dist-dir` копирует содержимое в `%APPDATA%\Kosmos\extensions\<id>\` где `id` взят из `manifest.json` source.
- **AC5 (CLI install — atomic overwrite)**: установка поверх существующей user-папки не оставляет mixed-state даже при прерывании (verify через temp dir подход).
- **AC6 (CLI uninstall)**: `ext:uninstall <id>` удаляет `%APPDATA%\Kosmos\extensions\<id>\`, после чего bundled версия снова видна.
- **AC7 (listExtensions dedup)**: если у extension'а есть и dev/user-installed, и bundled — `listExtensions()` возвращает один manifest (тот, что выиграл по priority).
- **AC8 (typecheck)**: `bun run --cwd apps/kepler-shell typecheck` зелёный.

## Файлы

**Modify:**

- `apps/kepler-shell/electron/extension-host.ts` — заменить `resolveExtensionsRoot()` (single) на `resolveExtensionRoots()` (array) + `resolveExtensionDir(id)` lookup.
- `apps/kepler-shell/package.json` — добавить scripts `ext:install`, `ext:uninstall`.

**Create:**

- `apps/kepler-shell/scripts/install-extension.mjs`
- `apps/kepler-shell/scripts/uninstall-extension.mjs`
- `docs-site/concepts/extension-installer.md`

**Update docs:**

- `docs-site/concepts/extension-host.md` — секция Resolution order.
- `docs-site/apps/kepler.md` — упоминание новых команд.
- `docs-site/apps/kepler-roadmap.md` — auto-update, `.kext` format, UI manager в потом.

## Связь с Kosmos planning

Когда Phase 6 Kosmos host'а будет на повестке — `%APPDATA%\Kosmos\extensions\` уже будет нужным path layout (Kosmos станет писать туда же при auto-update). Сейчас закладываем именно этот path, чтобы потом не мигрировать.
