# Extension installer

::: tip Статус — MVP (2026-05-14)
CLI install / uninstall в writable location, resolution chain в `extension-host.ts` поднимает user-installed копию выше bundled. **Не входит в MVP**: auto-update, `.kext` пакетный формат, UI manager в Kepler settings, code signing — см. [Kepler Roadmap](/apps/kepler-roadmap).
:::

## Зачем это нужно

До MVP extensions (`extensions/*`) уезжали в Kepler installer через `extraResources` в `electron-builder` конфиге. Обновить **один** extension означало пересобрать и переустановить весь shell. Это блокировало нормальный flow «итерация по конкретному приложению, не трогая launcher».

После MVP:

- Built-ins (Dashboard, Horologion, Delphi, Arrancador) **продолжают ехать** с Kepler installer'ом — это безопасный дефолт «свежеустановленный Kepler — всё работает».
- Поверх можно положить user-installed копию в `%APPDATA%\Kosmos\extensions\<id>\`. Resolution chain в [extension-host.ts](/concepts/extension-host#текущая-реализация-loader-а) поднимает её первой, перекрывая bundled.
- Удаление user-папки откатывает на bundled. Реверсивно.

## Layout

```text
<APPDATA>/Kosmos/extensions/<id>/        ← user-installed (writable, override)
  manifest.json
  dist/
  icon.png

<Kepler install dir>/resources/extensions/<id>/   ← bundled (read-only)
  manifest.json
  dist/
  icon.png
```

`<APPDATA>` — platform-зависимая:

| OS | Path |
|---|---|
| Windows | `%APPDATA%\Kosmos\extensions\` |
| macOS | `~/Library/Application Support/Kosmos/extensions/` |
| Linux | `$XDG_CONFIG_HOME/Kosmos/extensions/` (default `~/.config/...`) |

Resolution chain — см. [extension-host.ts](/concepts/extension-host#текущая-реализация-loader-а).

## CLI

```powershell
# Install: копирует <path-to-extension-dir> → <APPDATA>/Kosmos/extensions/<id>/
# <path-to-extension-dir> должен содержать manifest.json, dist/, icon.png
# <id> берётся из manifest.json
bun run --cwd shell ext:install <path-to-extension-dir>

# Uninstall: удаляет <APPDATA>/Kosmos/extensions/<id>/.
# Bundled версия (если есть) поднимется автоматически на следующем openExtension(id).
bun run --cwd shell ext:uninstall <id>
```

Скрипты — `shell/scripts/install-extension.mjs` / `uninstall-extension.mjs`. Не зависят от Electron, могут быть запущены вне launcher'а (build script / CI / ручной dev flow).

### Atomic install

Install выполняется в 4 шага, чтобы не оставить mixed-state при прерывании:

1. `cpSync(source, "<target>.tmp-<stamp>", { recursive: true })` — копия в соседнюю tmp папку.
2. Если target существует — `renameSync(target, "<target>.old-<stamp>")` (бывшая копия в сторону).
3. `renameSync("<target>.tmp-<stamp>", target)` — атомарный switch.
4. `rmSync("<target>.old-<stamp>", { recursive: true, force: true })` — cleanup.

При ошибке после шага 1 — best-effort rollback (вернуть `.old` → target, удалить `.tmp`).

## Что НЕ входит в MVP

::: danger Критический gap MVP — persistent user data
Сейчас install **полностью заменяет** папку `<APPDATA>/Kosmos/extensions/<id>/`. Если extension писал что-то внутрь своей папки (settings.json, кеш, window-state.json), при reinstall эти файлы **уничтожаются**. Для production-rollout это блокер.

Решение запланировано отдельно: разделить «код extension'а» и «user data extension'а» на два независимых path:

```
<APPDATA>/Kosmos/
├── extensions/<id>/         ← код (manifest, dist, icon) — replaceable, трогает install
└── extensions-data/<id>/    ← user data (settings, window-state, кеш) — install НЕ трогает
```

Window state (size/position BrowserWindow'а) Kepler shell сохранит сам в `extensions-data/<id>/window-state.json` по `close` event, восстановит при `openExtension`. Settings extension'а — через новый preload API `window.kepler.userData.{readFile, writeFile, readJson, writeJson}`.

Сейчас (MVP) extension'ы Dashboard / Horologion / Delphi / Arrancador **не пишут** ничего в свою папку — у них нет API для этого, всё user-state хранится в Kepler shell userData (`<userData>/kepler-shell-settings.json`) или в ARK через `kepler.ark.request`. Поэтому reinstall в текущем MVP **безопасен**. Но это совпадение — как только появится первый extension со своими файлами, gap начнёт срабатывать.

См. [Kepler Roadmap → Phase 10 → Persistent extension user data](/apps/kepler-roadmap#phase-10).
:::

Помимо persistent user data, в MVP отсутствуют:

- **Auto-update** — checker для новых версий. Сейчас юзер сам запускает `ext:install` со свежим dir.
- **`.kext` пакетный формат** — единый файл (zip с manifest + dist + icon), который Kepler ассоциирует как known mime type и устанавливает по двойному клику. Сейчас install принимает только директорию.
- **UI manager** — страница «Расширения» в Kepler settings со списком installed и кнопками install/remove. Сейчас всё через CLI.
- **Code signing / manifest validation** — проверка подписей издателя, schema validation manifest'а, capability declarations. Сейчас install верит источнику.

См. [Kepler Roadmap](/apps/kepler-roadmap) — пункты собраны в section «Extension installer / store».

## Связанные документы

- [Extension host](/concepts/extension-host) — resolution chain и loader.
- [Extension dev mode](/concepts/extension-dev-mode) — Vite HMR (отдельный канал от installer).
- [Kepler Roadmap](/apps/kepler-roadmap) — auto-update, `.kext`, UI manager в потом.
