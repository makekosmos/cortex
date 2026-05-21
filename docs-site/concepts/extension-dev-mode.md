# Extension developer mode

::: tip Зачем
Расшить DX миграции и работы над extension'ами: вместо «правка → rebuild extension → restart Kepler → открыть окно заново» — Vite HMR. Правишь `.vue`, окно extension'а перерисовывается без рестарта Kepler shell или backend'а.
:::

## Архитектура: probe-based auto-detect

Раньше extension dev mode требовал тройной opt-in (env var + Settings toggle + ручной `dev:extensions`). С 2026-05-19 — **автоматически**:

1. **`bun run --cwd shell dev`** запускает Vite dev server'ы для всех extension'ов параллельно (`shell/scripts/dev.mjs` → `dev-extensions.mjs`). Это дефолт; opt-out — `KEPLER_DEV_EXTENSIONS=0`.
2. **`openExtension(id, route?)`** при каждом вызове делает **TCP probe** `localhost:<manifest.devPort>` (timeout 500ms, кэш «alive» — 10s):
   - Порт отвечает → грузим с `http://localhost:<devPort>/` (Vite HMR).
   - Порт не отвечает → fallback на `dist/index.html` (warning в console).
3. **В production** (`VITE_DEV_SERVER_URL` не выставлен) probe не запускается совсем — packaged Kepler **всегда** грузит extension'ы из `dist/`.

Этот подход устраняет три проблемы прошлого дизайна:

- **Двойной opt-in** (env + setting) — теперь zero-config в dev.
- **Пустые окна при упавшем dev server** — graceful fallback на dist.
- **Footgun с persisted setting** — раньше `developerMode: true` в settings оставался после dev-сессии, в installed Kepler ломал загрузку extension'ов (localhost:5184 в проде → пустое окно).

## Workflow

```powershell
# Один терминал. Всё стартует автоматически:
bun run --cwd shell dev
```

- Shell, backend, и Vite dev servers всех extension'ов поднимаются параллельно.
- Edit `.vue` файла в `extensions/<id>/src/` → Vue HMR обновляет компонент в открытом extension window **без reload** окна.
- F12 в любом extension window — toggle DevTools (detached, не блокирует extension).
- DevTools auto-open на extension windows — dev-only поведение (probe вернул `dev-server`).

Если по какой-то причине Vite dev server для конкретного extension'а упал (порт битый, конфликт, OOM), окно при следующем open откроется из `dist/` с warning'ом `[kepler-shell] extension '<id>' dev server :<port> не отвечает — fallback на dist`. Перезапусти `bun run dev` чтобы поднять серверы заново.

## Раскладка портов

Каждому extension'у назначен фиксированный порт (см. `manifest.json::devPort`):

| Extension  | devPort |
| ---------- | ------- |
| Dashboard  | 5180    |
| Horologion | 5181    |
| Delphi     | 5182    |
| Arrancador | 5183    |
| Eden       | 5184    |

`shell/scripts/dev-extensions.mjs` поднимает по одному Vite dev server'у на порт, используя `extensions/<id>/vite.config.mjs`.

## Manifest

Поле `devPort` — **optional**. Если отсутствует, extension никогда не грузится с Vite — только из dist.

```json
{
  "id": "eden",
  "name": "Eden",
  "entryHtml": "dist/index.html",
  "devPort": 5184
}
```

## Probe

`shell/electron/extension-host.ts::probeExtensionDevServer(port)`:

```ts
const PROBE_TIMEOUT_MS = 500;
const PROBE_ALIVE_CACHE_TTL_MS = 10_000;
// TCP connect → success в пределах 500ms = alive.
// Cache «alive» на 10s чтобы не дёргать порт на каждый open.
// «Dead» не кэшируется — всегда перепроверяем (Vite мог подняться).
```

Стоимость в hot path: 0 (cached). Стоимость при cache-miss: ≤500ms (обычно <5ms localhost). Probe выполняется внутри `await` в `openExtensionImpl`, поэтому добавляет 0-5ms к user-visible latency открытия extension окна (только когда cache холодный или dev server недоступен — там 500ms таймаута).

## Concurrency

`openExtension(id, route)` использует in-flight Map для дедупликации параллельных вызовов — два быстрых invoke на один id вернут одну и ту же Promise<void>, не создадут двух BrowserWindow'ов. Раньше (синхронная функция) гонка была невозможна по построению; с async probe она появилась бы без явного дедупа.

## Settings toggle

Settings window kepler-shell имеет checkbox «Developer Mode» в `%APPDATA%\Kosmos\kepler-shell-settings.json`:

```json
{ "developerMode": true }
```

**Source resolution extension'ов больше НЕ зависит от этой настройки.** Setting сохранён для обратной совместимости и для UI-индикаторов «вы в dev mode», но реальное поведение определяется probe'ом. Удалять setting не торопимся — возможно, в будущем понадобится «force dev» override (например, чтобы запретить fallback на dist в чисто-HMR сценариях).

## Caveats

- Extension dev server слушает HTTP на `127.0.0.1` без auth — допустимо, single-user assumption + порты bound to loopback. Не выставляй наружу.
- HMR требует WebSocket extension renderer → Vite dev server. Локальный firewall с агрессивными правилами может блокировать `ws://127.0.0.1:<port>/`.
- Если поправил `electron/extension-host.ts` или другой main-process код — vite-plugin-electron перезапустит Electron автоматически (renderer'ы потеряются). HMR работает только в пределах renderer'ов.
- Probe ловит «порт открыт TCP-уровнем», не «Vite готов отдавать HTML». В момент cold-start Vite между `bind` и `ready` есть зазор ~50-200ms. Probe вернёт alive, но первый запрос может failure'нуться. На практике extension Vite servers стартуют до того как пользователь успевает вызвать команду — race пренебрежимо мал.

## Code refs

| Файл                                | Что                                                                                            |
| ----------------------------------- | ---------------------------------------------------------------------------------------------- |
| `shell/electron/extension-host.ts`  | `probeExtensionDevServer`, `resolveExtensionSource`, `openExtension` (async + inflight dedupe) |
| `shell/scripts/dev.mjs`             | Orchestrator: default-on extension Vite servers, opt-out `KEPLER_DEV_EXTENSIONS=0`             |
| `shell/scripts/dev-extensions.mjs`  | Per-extension Vite dev server spawn                                                            |
| `shell/vite.extensions.config.mjs`  | Vite config для extension build (one-shot dist)                                                |
| `extensions/<id>/vite.config.mjs`   | Per-extension vite config (HMR server, alias)                                                  |
| `shell/electron/settings-window.ts` | `developerMode` setting (legacy, UI hint only)                                                 |

## См. также

- [Extension host](/concepts/extension-host) — архитектура loader'а целиком.
- [Kepler Roadmap](/apps/kepler-roadmap) — статус Phase 6+.
