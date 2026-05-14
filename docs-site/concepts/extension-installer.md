# Extension installer

::: tip Статус — MVP (2026-05-14)
CLI install / uninstall в writable location, resolution chain в `extension-host.ts` поднимает user-installed копию выше bundled. Persistent user data split (`extensions-data/<id>/`) — сделан в той же версии (2026-05-14): install не трогает user data, uninstall по умолчанию тоже не трогает; флаг `--purge-data` удаляет и user data. **Не входит в MVP**: auto-update, `.kext` пакетный формат, UI manager в Kepler settings, code signing — см. [Kepler Roadmap](/apps/kepler-roadmap).
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
# User data в <APPDATA>/Kosmos/extensions-data/<id>/ по умолчанию сохраняется.
bun run --cwd shell ext:uninstall <id>

# Uninstall + очистка user data: удаляет и <APPDATA>/Kosmos/extensions-data/<id>/.
bun run --cwd shell ext:uninstall <id> --purge-data
```

Скрипты — `shell/scripts/install-extension.mjs` / `uninstall-extension.mjs`. Не зависят от Electron, могут быть запущены вне launcher'а (build script / CI / ручной dev flow).

### Atomic install

Install выполняется в 4 шага, чтобы не оставить mixed-state при прерывании:

1. `cpSync(source, "<target>.tmp-<stamp>", { recursive: true })` — копия в соседнюю tmp папку.
2. Если target существует — `renameSync(target, "<target>.old-<stamp>")` (бывшая копия в сторону).
3. `renameSync("<target>.tmp-<stamp>", target)` — атомарный switch.
4. `rmSync("<target>.old-<stamp>", { recursive: true, force: true })` — cleanup.

При ошибке после шага 1 — best-effort rollback (вернуть `.old` → target, удалить `.tmp`).

## Persistent user data

::: info Persistent user data — split с 2026-05-14

Код и user data extension'а разделены на два path:

```
<APPDATA>/Kosmos/
├── extensions/<id>/         ← код (manifest, dist, icon) — install полностью заменяет
└── extensions-data/<id>/    ← user data — install НЕ трогает
    ├── settings.json         (extension сам пишет через preload API)
    ├── window-state.json     (Kepler shell сам сохраняет на close)
    └── ...                   (любые user files extension'а)
```

- **Install** трогает только `extensions/<id>/`. `extensions-data/<id>/` сохраняется через все обновления.
- **Uninstall** (`bun run --cwd shell ext:uninstall <id>`) по умолчанию удаляет только код, user data preserved.
- **Uninstall с очисткой**: `bun run --cwd shell ext:uninstall <id> --purge-data` удаляет и code, и user data.
- **Preload API для extension'ов**: `window.kepler.userData.{readJson, writeJson, readFile, writeFile, path}` — см. [Extension host → User data](/concepts/extension-host#user-data).
- **Window state**: размер и положение окна каждого extension'а Kepler shell сохраняет автоматически в `extensions-data/<id>/window-state.json` по `close` / debounced `resized`/`moved`. Extension ничего не делает.
:::

## Что НЕ входит в MVP

В MVP отсутствуют:

- **Auto-update** — checker для новых версий. Сейчас юзер сам запускает `ext:install` со свежим dir.
- **`.kext` пакетный формат** — единый файл (zip с manifest + dist + icon), который Kepler ассоциирует как known mime type и устанавливает по двойному клику. Сейчас install принимает только директорию.
- **UI manager** — страница «Расширения» в Kepler settings со списком installed и кнопками install/remove. Сейчас всё через CLI.
- **Code signing / manifest validation** — проверка подписей издателя, schema validation manifest'а, capability declarations. Сейчас install верит источнику.

См. [Kepler Roadmap](/apps/kepler-roadmap) — пункты собраны в section «Extension installer / store».

## Связанные документы

- [Extension host](/concepts/extension-host) — resolution chain и loader.
- [Extension dev mode](/concepts/extension-dev-mode) — Vite HMR (отдельный канал от installer).
- [Kepler Roadmap](/apps/kepler-roadmap) — auto-update, `.kext`, UI manager в потом.
