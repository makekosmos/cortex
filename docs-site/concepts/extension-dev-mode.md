# Extension developer mode

::: tip Зачем
Раскручивать DX миграции апок в extensions: вместо «rebuild extension → restart Kepler → открыть окно заново» нужен Vite HMR — поправил `.vue`, окно extension'а перерендерилось без рестарта host'а. Паттерн как `ray develop` у Raycast.
:::

## Цель

Hot-reload extension'ов через Vite dev servers. Изменения в `extensions/<id>/src/` → автоперезагрузка extension renderer без рестарта Kepler shell или backend'а.

## Включение

Extension HMR активируется **только** через явный toggle:

- **Settings → Developer Mode** toggle (persist в `%APPDATA%\Kosmos\kepler-shell-settings.json`, см. ниже).
- **И** параллельно поднятые Vite dev servers — `bun run --cwd shell dev:extensions`.

::: warning KEPLER_DEV больше НЕ активирует extension dev mode
Env var `KEPLER_DEV=1` (которую автоматом выставляет `bun run --cwd shell dev`) раньше включала extension HMR. Это ломало dev-сессию без поднятых dev server'ов — extension окна получали URL `http://localhost:5180/...` и были пустые. Сейчас `KEPLER_DEV=1` влияет только на shell-level dev (DevTools шелла, dev URL шелла); extension loader смотрит исключительно на `developerMode: true` в `kepler-shell-settings.json`. Toggle в Settings UI **отображается** включенным при `KEPLER_DEV=1` (для консистентности индикатора), но это только UI-индикатор — реальное поведение управляется JSON-настройкой.
:::

При включении extension-host резолвит `entryHtml` не из `dist/`, а из `http://localhost:<devPort>/`.

## Workflow

Два терминала, параллельно:

```powershell
# Terminal 1 — Vite dev servers для каждого extension'а
bun run --cwd shell dev:extensions

# Terminal 2 — Kepler shell (KEPLER_DEV=1 выставляется автоматом, но extension HMR
# требует ещё включить Developer Mode toggle в Settings UI — единоразово)
bun run --cwd shell dev
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

Resolver `openExtension(id, route?)` в `shell/electron/extension-host.ts`:

- Если `developerMode: true` в `kepler-shell-settings.json` **и** в манифесте есть `devPort` → `BrowserWindow.loadURL('http://localhost:<devPort>/#<route>')`.
- Иначе → fallback на `loadFile(<root>/<id>/<entryHtml>, { hash: route })` из bundled dist.

## Settings toggle

Settings window kepler-shell имеет checkbox «Developer Mode». Значение хранится в `%APPDATA%\Kosmos\kepler-shell-settings.json`:

```json
{
  "developerMode": true
}
```

- Toggle включает/выключает dev-resolution **на следующий** `openExtension(id)`. Уже открытые окна не перезагружаются автоматически.
- `developerMode` в JSON — **единственный** источник правды для extension loader'а.
- Settings UI **отображает** `KEPLER_DEV=1 || developerMode` (через IPC `kepler:settings:developer-mode:get`), чтобы toggle в dev-сессии не выглядел случайно «выключенным». Это исключительно UI-индикатор — переключатель надо явно щёлкнуть, чтобы JSON обновился и extension loader увидел изменение.

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
