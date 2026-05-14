# Extension developer mode

::: tip Зачем
Раскручивать DX миграции апок в extensions: вместо «rebuild extension → restart Kepler → открыть окно заново» нужен Vite HMR — поправил `.vue`, окно extension'а перерендерилось без рестарта host'а. Паттерн как `ray develop` у Raycast.
:::

## Цель

Hot-reload extension'ов через Vite dev servers. Изменения в `extensions/<id>/src/` → автоперезагрузка extension renderer без рестарта Kepler shell или backend'а.

## Включение

Один из вариантов:

- **Env var** `KEPLER_DEV=1` при запуске kepler-shell.
- **Settings → Developer Mode** toggle (persist в JSON, см. ниже).

При включении extension-host резолвит `entryHtml` не из `dist/`, а из `http://localhost:<devPort>/`.

## Workflow

Два терминала, параллельно:

```powershell
# Terminal 1 — Vite dev servers для каждого extension'а
bun run --cwd shell dev:extensions

# Terminal 2 — Kepler shell с включённым dev режимом
$env:KEPLER_DEV = "1"; bun run --cwd shell dev
```

После этого:

- Edit `.vue` файла в `extensions/<id>/src/` → Vue HMR обновляет компонент в открытом extension window **без reload** окна.
- F12 в любом extension window — toggle DevTools (detached, не блокирует extension).
- DevTools auto-open на launcher window — тоже dev-only поведение.

## Раскладка портов

Каждому extension'у назначен фиксированный порт, чтобы избежать конфликтов и сделать manifest предсказуемым:

| Extension | devPort |
|---|---|
| Dashboard | 5180 |
| Horologion | 5181 |
| Delphi | 5182 |
| Arrancador | 5183 |

`bun run --cwd shell dev:extensions` поднимает по одному Vite dev server'у на каждый порт (см. `shell/vite.extensions.config.mjs`).

## Manifest

Поле `devPort` — **optional** в extension manifest:

```json
{
  "id": "dashboard",
  "name": "Dashboard",
  "entryHtml": "dist/index.html",
  "devPort": 5180
}
```

Resolver `openExtension(id)` в `shell/electron/extension-host.ts`:

- Если `KEPLER_DEV=1` **и** в манифесте есть `devPort` → `BrowserWindow.loadURL('http://localhost:<devPort>/')`.
- Иначе → fallback на `loadFile(<root>/<id>/<entryHtml>)` из bundled dist.

## Settings toggle

Settings window kepler-shell имеет checkbox «Developer Mode». Значение хранится в `%APPDATA%\Kosmos\kepler-shell-settings.json`:

```json
{
  "developerMode": true
}
```

- Toggle включает/выключает dev-resolution **на следующий** `openExtension(id)`. Уже открытые окна не перезагружаются автоматически.
- Без env-перменной значение из JSON — единственный источник.
- Если **и** env var выставлен, **и** JSON true — оба эквивалентны, флаг ON.

Код: `shell/electron/settings-window.ts` (loader/saver + IPC handler), `shell/src/views/SettingsView.vue` (UI).

## Caveats

- Extension dev server слушает HTTP на `localhost` без auth — допустимо, потому что Kosmos single-user assumption и порты bound to loopback. Не выставляй наружу.
- HMR требует WebSocket connection extension renderer → Vite dev server. Локальный firewall с агрессивными правилами может блокировать `ws://localhost:<port>/`.
- DevTools auto-open на launcher и extension windows — поведение **dev-only**. В production build (`bun run build`) флага нет, DevTools закрыты.
- Если поправил `electron/extension-host.ts` или другой main-process код — нужен **рестарт** kepler-shell. HMR работает только для renderer.

## Code refs

| Файл | Что |
|---|---|
| `shell/electron/extension-host.ts` | `openExtension(id)` resolver: dev URL vs dist file |
| `shell/vite.extensions.config.mjs` | Vite dev server config per extension (порт-маппинг) |
| `shell/electron/settings-window.ts` | Developer Mode setting (load/save/IPC) |
| `shell/src/views/SettingsView.vue` | Settings UI с Developer Mode toggle |

## См. также

- [Extension host](/concepts/extension-host) — архитектура loader'а целиком.
- [Kepler Roadmap](/apps/kepler-roadmap) — Phase 4 статус миграции.
- [RAM benchmarks](/concepts/ram-benchmarks) — почему всё это вообще делается.
