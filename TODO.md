# TODO

## UX polish (отложенные)

- [ ] **Полоска загрузки снизу при in-app update.** Когда autoUpdater качает новую версию через UI (Settings → Обновление), показывать тонкий progress bar внизу окна Kepler с процентом download'а. Сейчас обновление триггерится из Settings, но визуальной обратной связи о ходе скачивания нет — пользователь не знает, идёт ли загрузка. Источник прогресса: `electron-updater` `download-progress` event (см. `platform/desktop/electron/autoupdater-host.ts`). Renderer-side hook через IPC. Делать когда руки дойдут — не блокирует другие фичи.

## Kepler Extension API v2 — Raycast-shaped UX (2026-05-23)

**Цель**: дать extensions DX как у Raycast (inline render в launcher window, `<KList>`/`<KDetail>`/`<KForm>`/`<KActionPanel>`, `useNavigation` push/pop, clipboard/toast/preferences) при сохранении Vue-стека и текущей архитектуры «extension = own process для legacy, inline mount для нового API».

**Не цель**: запуск существующих Raycast-extensions as-is. Причины — JSX/React runtime, лицензия `@raycast/api` (closed-source, нужна clean-room реализация), macOS-specific фичи (AppleScript, Frontmost App Bundle), AI/OAuth/BrowserExtension у Raycast живут в их облаке. Реалистичный максимум — 60–70% extension'ов при mechanical-port через codemod. Полный compat = очень большой проект, не рекомендую.

### Стратегические решения (зафиксировать в `.agent/tasks/<DATE>-kepler-ext-api-v2/spec.md` перед стартом)

1. **DX-цель**: Vue-native с Raycast-shape компонентов (**рекомендуется**) vs React-runtime внутри Electron (отвергается).
2. **Process isolation для inline extensions**: `<webview>` (изоляция, ограниченный IPC) vs direct mount в launcher process (быстро, но extension видит DOM launcher'а). Влияет на security model.
3. **Permission model**: сделать **ДО** расширения API surface. Manifest declares capabilities (`clipboard`, `shell.open`, `applications`, `oauth`, …), backend gates `ark.request` по ним. Без этого расширение surface = security hole.
4. **MVP scope первой итерации**: `<KList>` + `<KDetail>` + `<KActionPanel>` + `useNavigation` + `clipboard`/`showToast`/`environment` + `kind: "inline"` mode + permission declaration. Остальное — отдельные подзадачи.

### POC до полноценной работы

Минимум для валидации концепции: новый sample extension под `extensions/<id>/` с `manifest.kind: "inline"`, монтируется в slot launcher window'а вместо открытия отдельного `BrowserWindow`, использует `<KList>` + `<KDetail>` из `@kosmos/visuals`, ESC → popToRoot, ⌘K → action panel, Enter → push view. Если POC работает быстро и приятно — расширяем API surface. Если упирается в архитектуру (IPC через `<webview>` медленный, sandbox vs DX trade-off) — пересматриваем модель.

### Gap-list (отсортировано лёгкое → тяжёлое)

| #      | Что                                                                                                 | Сложность     | Группа            |
| ------ | --------------------------------------------------------------------------------------------------- | ------------- | ----------------- |
| 1      | **Permission/capability в manifest + backend gating `ark.request`**                                 | средне        | **prerequisite**  |
| 2      | `kepler.clipboard` (Electron `clipboard`: text/HTML/files)                                          | низкая        | preload           |
| 3      | `kepler.environment` (`isDevelopment, supportPath, assetsPath, extensionName, version, launchType`) | низкая        | preload           |
| 4      | `kepler.shell.{openExternal,open,trash}`                                                            | низкая        | preload           |
| 5      | `kepler.toast` / `kepler.hud` / `kepler.confirmAlert` (overlay в `@kosmos/visuals`)                 | низкая        | UI                |
| 6      | `kepler.localStorage` key-by-key поверх `userData`                                                  | низкая        | storage           |
| 7      | `kepler.cache` (TTL'd)                                                                              | низкая        | storage           |
| 8      | **Manifest: `preferences[]` + `getPreferenceValues()` + Settings UI**                               | средне        | core              |
| 9      | **Manifest: per-command `arguments[]` + launcher prompt form**                                      | средне        | core              |
| 10     | **`<KList>` (keyboard nav + search filter + sections + dropdown)**                                  | высокая       | UI core           |
| 11     | `<KDetail>` (markdown + metadata side panel)                                                        | низкая        | UI                |
| 12     | `<KForm>` (TextField/TextArea/Dropdown/TagPicker/DatePicker/Checkbox/Submit)                        | средне        | UI                |
| 13     | **`<KActionPanel>` + `<KAction.*>` + ⌘K popup + keyboard shortcuts**                                | высокая       | UI core           |
| 14     | `<KGrid>`                                                                                           | низкая        | UI                |
| 15     | **`useKeplerNavigation()` push/pop stack**                                                          | средне        | core (под inline) |
| 16     | **`kind: "inline"` render mode в launcher**                                                         | средне        | core              |
| 17     | `kepler.applications.{getFrontmost,list,getDefault}` (Windows: PowerShell / win32 через Rust)       | средне        | native            |
| 18     | `kepler.keyboard` global shortcuts                                                                  | низкая        | native            |
| 19     | `kepler.launchExtension(id, cmd, args?)` (поднять internal в preload + permission)                  | низкая        | core              |
| 20     | `kepler.oauth.PKCEClient` (token store в `userData` + browser flow через custom protocol)           | средне        | auth              |
| 21     | `kepler.ai.ask` (proxy к настроенному провайдеру, stream через IPC)                                 | средне        | AI                |
| 22     | Background runtime: `mode: "no-view"` + `interval` scheduler                                        | высокая       | runtime           |
| 23     | Codemod `raycast-to-kepler` (mechanical port TSX → Vue SFC)                                         | средне        | tooling           |
| 24     | MenuBarExtra equivalent (tray-resident mini-renderer)                                               | высокая       | UI                |
| 25     | `kepler.browserExtension` (companion browser ext через WS)                                          | высокая       | отдельный проект  |
| ~~26~~ | ~~Полная React/JSX совместимость~~                                                                  | очень высокая | **не делаем**     |

### Текущее состояние Kepler extension API (что есть сейчас)

`window.kepler.*` (`platform/desktop/electron/extension-preload.ts:1-215` + `extension-host.ts:1108-1504`):

- `kepler.ark.request(op, params)` / `.subscribe(event, cb)` — generic RPC к Rust backend (главная рабочая лошадка)
- `kepler.window.*` — `close/minimize/maximize/toggleDockCorner/...` (Kepler-unique)
- `kepler.userData.*` — `readJson/writeJson/readFile/writeFile/path`
- `kepler.navigation.initialRoute()` + `.onNavigate(handler)` — host пушит route string
- `kepler.meta.id()`
- `kepler.host.invoke(action, payload)` — **stub**, всегда `false` (`extension-host.ts:1373-1376`)
- `kepler.backend.onReady/onDisconnected`
- `kepler.arrancador.*` — domain sugar над `ark.request("arrancador.*")`

Manifest: `id, name, version, keplerApiVersion, kind: "vue", entryHtml, devPort, width/height, keepAliveInBackground, commands: [{id, title, subtitle, kind, mode: "open"|"action"}]`.

### Mapping Raycast → Kepler (текущий статус)

| Raycast                                                  | Kepler сейчас                                    | Статус           |
| -------------------------------------------------------- | ------------------------------------------------ | ---------------- |
| UI компоненты (List/Detail/Form/Grid/ActionPanel/Action) | нет — extension рендерит свой Vue                | ❌               |
| Feedback (showToast/showHUD/confirmAlert)                | нет                                              | ❌               |
| Clipboard                                                | только `navigator.clipboard` (text)              | 🟡               |
| LocalStorage                                             | `userData.readJson/writeJson` (целые файлы)      | 🟡               |
| Cache                                                    | нет                                              | ❌               |
| Preferences declarative + `getPreferenceValues`          | нет                                              | ❌               |
| per-command `arguments`                                  | нет в manifest                                   | ❌               |
| `useNavigation` push/pop                                 | только route string push от host                 | 🟡               |
| `popToRoot/closeMainWindow`                              | `window.close()` (extension window, не launcher) | 🟡               |
| `getFrontmostApplication/getApplications`                | данные есть в `usage_tracker`, не выставлены     | ❌               |
| `shell.open/openExternal/trash`                          | нет                                              | ❌               |
| `OAuth.PKCEClient`                                       | нет                                              | ❌               |
| `AI.ask`                                                 | нет                                              | ❌               |
| `BrowserExtension`                                       | нет                                              | ❌               |
| `environment.*`                                          | только `userData.path()`                         | 🟡               |
| Icon/Color constants                                     | нет (есть design tokens в `@kosmos/visuals`)     | 🟡               |
| Keyboard.Shortcut global                                 | нет                                              | ❌               |
| `launchCommand` cross-extension                          | внутренний есть, в preload не выставлен          | 🟡               |
| `mode: "no-view"/"menu-bar"` + `interval`                | нет runtime'а                                    | ❌               |
| `@raycast/utils` hooks (useFetch/usePromise/useExec/...) | n/a — пишут руками                               | 🔵               |
| `ark.request/subscribe` (objects+sync RPC)               | есть                                             | 🔵 Kepler-unique |
| Per-extension window control (resize/dock/maximize)      | есть                                             | 🔵 Kepler-unique |

### Архитектурные различия (важные ограничения)

1. **Render model**. Raycast = одно launcher window, активная команда inline под search bar, navigation = стек React-элементов в том же окне. Kepler = `BrowserWindow` per extension. Под `kind: "inline"` нужно либо `<webview>` (process isolation, ограниченный IPC), либо direct mount (быстро, нет sandbox).
2. **UI framework**. Raycast = JSX/React. Kepler = Vue. Запускать TSX напрямую нельзя — разные runtime'ы. Vue-компоненты с тем же контрактом DX — реалистично.
3. **Process model**. Raycast spawn'ит команды как Node subprocess за ~50ms. Electron на Windows этого для BrowserWindow не даёт. Для inline через direct mount — быстро.
4. **Permission**. У Raycast — sandbox + capabilities из manifest. У Kepler **любой extension может вызвать любой `ark.request`** — нет gate'инга. Решить ДО расширения surface.
5. **Background/scheduled**. У Raycast `mode: "no-view"` + `interval` для headless. У Kepler `mode: "action"` есть в manifest, но extension runtime для headless нет.

### Процесс

Substantial-задача → **proof loop** обязательно. Структура:

```
.agent/tasks/<DATE>-kepler-ext-api-v2/
  spec.md          — стратегические решения 1–5 выше
  research/        — research от 2026-05-23 (этот блок + полный agent output в conversation)
  plan.md          — порядок реализации (MVP scope → расширение)
  acceptance.md    — критерии: новый sample-extension рендерится inline,
                     показывает <KList> с search, открывает <KDetail>,
                     <KAction.CopyToClipboard> работает, ESC → popToRoot.
```

### Ключевые ссылки

- Preload: `platform/desktop/electron/extension-preload.ts:1-215`
- Host handlers: `platform/desktop/electron/extension-host.ts:1108-1504` (ark proxy `:1117`, navigation `:1236`, window `:1243-1372`, userData `:1396-1441`)
- Manifest пример: `products/delphi/manifest.json`
- Roadmap (Phase 13): `docs-site/apps/kepler-roadmap.md:339-361`
- Distribution (Raycast-style two-repo update model): `docs-site/concepts/distribution.md:10`

## AI-first object graph (2026-05-19)

North star: всё есть объект, любые объекты связываются через `object_links`, AI-агенты — first-class consumer. Markdown — рендеринг для людей, не source of truth. См. memory `project_north_star_object_graph.md` и обсуждение от 2026-05-19.

Action items (приоритет сверху вниз):

- [ ] **MCP server поверх `@kosmos/ark`** — обёртка над существующим WS / command bus в `platform/runtime`. Эмитит JSON Schema из `object_types.schemaJson` как tool definitions. Подключается одним `claude mcp add` / Cursor / любой LLM-host. Снимает 90% галлюцинаций агента — schema-driven, не угадывание формата.
- [ ] **`content_md` projection в read/write API.** `get_object` опционально возвращает `content_md` (TipTap JSON → markdown через существующий `note_md` converter из Phase 7). `upsert_object` принимает `content_md` и парсит обратно в TipTap JSON. Source of truth остаётся `content_json`, но для агентов и для человеческого чтения наружу торчит markdown. Нужно достроить обратный конвертер markdown → TipTap, если ещё нет (export-only сейчас).
- [ ] **`get_neighborhood(id, depth)` ARK endpoint.** Возвращает subgraph: узлы (объекты) + рёбра (links) в радиусе N от заданного id. Сейчас агенту чтобы понять «что связано с этой заметкой» нужно `list_object_links` → фильтр → `get_objects_by_ids`. Один endpoint = multi-hop reasoning возможен в один tool-call.
- [ ] **Read-only markdown mirror** под `<data_dir>/notes-md/` (и аналогично для других «человекочитаемых» типов: `tasks-md/`?). Регенерируется на save через `note_md` converter. User видит файлы в Explorer, может grep / git commit / открыть в Obsidian для чтения. Контракт: правки в файлах **игнорируются** (или показываются как hint «создать новую заметку из этого файла»). Source of truth — ARK SQLite. Это закрывает психологический запрос на portability без двойного sync'а.
- [ ] **Conversational agent поверх ARK MCP (Hermes Agent? или тонкий собственный).** Идея — главный «собеседник» Kepler'а, с которым можно разговаривать (в т.ч. голосом через Kerux) и через которого делаются операции над графом: «закинь это в инбокс Delphi», «что я писал про X на прошлой неделе», «начни pomodoro на 25 минут». Кандидат — [Hermes Agent](https://github.com/NousResearch/hermes-agent) (NousResearch, MIT, MCP-compatible, self-improving, voice memo transcription, multi-channel gateway). Архитектура: MCP-фасад над `@kosmos/ark` (см. пункт выше) → Hermes Python sidecar под supervisor'ом `kepler-backend` → будущий Vue-extension под `extensions/<id>/` с chat UI через command bus → Kerux как голосовой транспорт (hotkey → whisper → текст в Hermes). Развилка перед стартом: **либо Hermes** (self-improving, multi-channel — Telegram/Discord/Slack бонусом, но Python в стеке + второй memory store параллельно ARK), **либо тонкий ассистент на AI SDK + Anthropic/OpenAI** (~неделя, без Python, единственный memory — ARK). MCP-фасад нужен в обоих сценариях, поэтому решение реверсируемое. Обсуждение от 2026-05-20.

Не делать:

- ❌ Переход на markdown как primary storage. Ломает typed propsJson + schema, ID-stable wikilinks, cross-type graph, единый sync invariant (HLC + version_vector + tombstones). См. [forbidden.md](docs-site/agents/forbidden.md) и memory `project_north_star_object_graph.md`.
- ❌ Block-level granularity (Logseq transclusion) через файлы. Если когда-то понадобится — реализуется как `block_obj` тип внутри object model.

## Legacy Note

The previous contents of this file described an older `core/ark/packages/ark/` Python/server-era plan. That is no longer the active ARK architecture.

Current ARK runtime documentation:

- [`crates/ark-core/README.md`](./crates/ark-core/README.md)
- Rust runtime: `crates/ark-core`
- Node/Electron SDK: `core/ark/packages/ark` (`@kosmos/ark`)
- Compatibility SDK `@arksync/node` больше не поддерживается как workspace; current TS SDK — `core/ark/packages/ark` (`@kosmos/ark`).
- Canonical desktop sidecar: `ark-core-rpc`

## Current ARK Priorities

- Keep app writes going through `ark-core-rpc` / `@kosmos/ark`.
- Keep direct Rust writers on `ark_core::db` helpers so sync state is updated consistently.
- Delphi tasks migrate automatically at app startup into `task_obj` records in the generic object model.
- Eden notes and custom typed notes are ARK objects (`note_obj` or their custom object type). Heart remains available for editor/vault-specific behavior and one-time migration/import/export work.
- Arrancador game writes, usage backfill, game hydration, usage summaries, and process search should prefer `@kosmos/ark`; read-only SQLite is only a fallback when the ARK runtime is unavailable.
- Dashboard is a read-only ARK inspector/analytics app. It may inspect selected ARK SQLite databases from Electron main, but must never write to ARK tables.
- Keep the smoke matrix in `docs/ARK-SMOKE-MATRIX.md` current and ensure every automated check uses an isolated test DB or temporary app-data path.
- Use `bun run ark:guard:writes` before changing app data services; direct app-service writes to ARK tables are forbidden.
- ARK runtime now has object query, usage process query, and game playtime summary endpoints. If read-only ARK SQL needs to be removed completely, continue replacing fallback paths with dedicated runtime analytics endpoints.
- Delphi legacy DB sidecar is removed; `task_obj` ARK objects are the task source of truth after startup migration.
