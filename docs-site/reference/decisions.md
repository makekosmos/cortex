# Журнал решений (ADR)

Архитектурные решения, оформленные как отдельные документы в `docs/`. Каждое решение — это «почему сделали именно так», чтобы через год можно было понять контекст.

## Список решений

### Delphi — legacy DB sidecar удалён

Источник: `docs/DELPHI-LEGACY-DB-DECISION.md`

Старый Delphi-specific Rust DB sidecar удалён. Текущая замена для shared task state — ARK object storage:

- task data: `objects` с `type_id = task_obj`
- typed/schema metadata: `object_types`
- relationships: `object_links`
- app access: `@kosmos/ark` / `ark-core-rpc`

Правила:

- Не паковать и не восстанавливать legacy Delphi DB sidecar (исторический путь — apps/delphi/ts/sidecar, удалён до Phase B).
- Не использовать old todo таблицы как long-term fallback после миграции.
- Delphi startup может читать legacy todos **только** для миграции в `task_obj`.
- После миграции `task_obj` — источник правды.

### Eden Heart vs ARK boundary

Источник: `docs/EDEN-HEART-ARK-BOUNDARY.md`

Eden Heart **остаётся**, не удалён, потому что Eden notes — ARK объекты. Разделение:

- **ARK** владеет shared data identity и syncable state: `object_types`, `objects`, `object_links`.
- **Eden Heart** владеет тяжёлой editor/vault-local работой: vault import/export, one-time migration source reads, editor-oriented трансформации, будущий специализированный поиск/индексация.
- Новые Eden note writes идут в ARK objects. Heart **не** permanent fallback для shared note identity/state после startup migration.

Search-решение:

- **ARK search** — поиск по `objects.title`, `objects.content_json`, `objects.props_json`.
- **Heart search** — будущий Eden-specific Rust индекс для editor/vault контента, если ARK object search недостаточно.
- **Текущий дефолт**: shared notes ищутся через ARK; Heart остаётся как опция позже.

Текущее runtime правило:

- `loadEntry`, `listEntries`, `listNoteTypes`, `searchEntries` читают **только** ARK объекты/types.
- Heart entry/type reads — только startup migration, не normal read paths.

### Read-only SQL boundary

Источник: `docs/ARK-READONLY-SQL-BOUNDARY.md`

Две независимые политики:

1. **Write rule** — жёсткий: app TS services **не** пишут напрямую в ARK таблицы.
2. **Read inspection** — мягче:
   - Dashboard — read-only inspector, может открывать любую выбранную ARK SQLite-БД.
   - Arrancador — `@kosmos/ark` сначала; raw SQLite допустим как fallback когда runtime недоступен.
   - Renderer — никогда не открывает SQLite напрямую.

Будущий шаг — заменить оставшиеся fallback SQLite paths специализированными ARK endpoints. Уже добавлены:

- `objects.listByType` → `list_objects_by_type`
- `objects.getMany` → `get_objects_by_ids`
- `usage.processes.recent` → `list_recent_usage_processes`
- `usage.processes.search` → `search_usage_processes`
- `usage.gamePlaytime.summary` → `get_usage_game_playtime_summary`

### 2026-05-14 — Brand swap Kepler ↔ Kosmos

Источник: `docs/MIGRATION-2026-05-14-brand-swap.md`

Поменяли семантику бренда:

- **Kepler** теперь — имя **лаунчера** и его shell (`platform/desktop/`, `platform/runtime/`).
- **Kosmos** теперь — имя **экосистемы / монорепо** (`@kosmos/ark`, `@kosmos/visuals`, ARK runtime, документация).

Раньше было наоборот. Все references в коде, конфигах, документации и токенах прошли через `scripts/migrate-kepler-to-kosmos.ps1`. Гард — `scripts/check-swap-completeness.ps1`.

### 2026-05-14 — Apps остаются standalone .exe + shared backend

Решено **не** мигрировать приложения в extensions лаунчера (Phase 1-3 plan отброшен). Каждое приложение по-прежнему — независимый Electron `.exe` со своим окном и пакетом. Kepler-shell вызывает их через command bus; общий backend (`platform/runtime/`) хостит command registry и WS server.

Причина: extension model в Phase 1 PoC показал нарастающую сложность (разделяемый renderer, конфликты CSS-токенов, packaging) при минимальной выгоде. Standalone-распространение проще и сохраняет user expectation «отдельная иконка в Start menu на каждое приложение».

### 2026-05-14 — Command bus как primary integration primitive

Apps **регистрируют** свои commands в shared backend (через `@kosmos/ark` SDK), launcher **invoke**'ает их. Это заменяет более ранний план «ARK FTS5 search в launcher» — поиск в Kepler-shell теперь идёт по зарегистрированным командам, а не по индексу заметок/задач.

- Wire format: flat events `{event: "...", ...fields}` (не nested).
- Registration в `kepler-mode` only, под `try/catch`.

### 2026-05-14 — Delphi extension остался на Tailwind v4

После миграции в extension Delphi сохранил Tailwind v4 (`@tailwindcss/vite` плагин подключён в `vite.config.mjs` extension'а). Переписывать UI на plain CSS / kosmos-visuals токены — отдельная Phase 9 задача, не выполняется попутно. Удалять Tailwind сейчас опасно: ломает существующие классы во всех Delphi-страницах без эквивалентной замены.

Open question — после Phase 9 перевести Delphi на kosmos-visuals токены и удалить Tailwind dependency.

### 2026-05-14 — Arrancador native scanner остался в legacy

Native game scanner Arrancador'а (сканирование Steam / Epic / GOG библиотек, запуск .exe) остался в **legacy standalone Electron main**, не мигрирован в extension. Причина: extension renderer не имеет доступа к node API, а scanner требует `child_process` и FS-сканирование с правами user'а.

Phase 5+ план — либо вынести scanner в `kepler-backend` (Rust) с capability-API через `@kosmos/ark`, либо в отдельный sidecar в `kepler-shell` electron main. До этого Arrancador-extension содержит только UI subset (LayoutPage + GameCard), все catalogue / scan / launch операции — stub'ы или disabled.

### 2026-05-14 — Extension icons через base64 data URI + mtime cache

Иконки extension'ов (`extensions/<id>/icon.png`) передаются в renderer launcher'а как `data:image/png;base64,...` (через `extensionIconDataUri(id)`), а не как `file://path/to/icon.png`. Причины:

- **Security**: `file://` URLs из renderer Electron не любит (нужен webSecurity off либо protocol scheme), data URI работает прозрачно.
- **Hot reload**: in-memory cache с mtime invalidation позволяет менять иконки без restart'а shell'а.
- **No leak path**: renderer не получает абсолютные пути к ресурсам.

Минус — `base64` раздувает payload на ~33%. Для иконок 64×64 PNG это ~5 KB, приемлемо.

См. [Extension host → App icons](/concepts/extension-host#app-icons).

### 2026-05-14 — Launcher window: fixed-size 720×460

Лаунчер Kepler — окно фиксированного размера 720×460. Animated resize (per-frame) отброшен после экспериментов: Win32 не успевает синхронно прокидывать события, окно дёргается. Решение — фиксированный размер; expand/collapse состояния выражаются через layout внутри renderer, не через resize окна.

### 2026-05-24 — Dictation Phase 1: облачный Groq, не локальный whisper.cpp

Источник: `concepts/dictation.md` + `.agent/tasks/2026-05-24-dictation/spec.md`.

Phase 1 диктации — **только облачная транскрипция через Groq Cloud** (whisper-large-v3-turbo). Audio capture — в renderer'е через Web Audio API; backend получает готовый WAV в `dictation.submit_audio`. Локальные модели (whisper.cpp через `whisper-rs`, Parakeet V3 через onnx-runtime) — отдельный proof-loop Phase 2.

Причина: MVP за разумные часы. Cloud-only требует ~12 файлов (Rust dictation module + Electron pill + Vue settings); локальные модели добавляют download manager, model storage, CUDA acceleration setup (на Windows капризный, см. [whisper.cpp#2857](https://github.com/ggml-org/whisper.cpp/issues/2857)), AudioWorklet streaming. Это ещё ~3-5× scope'а и требует отдельной валидации производительности per-модель.

Sub-решения:

- **API key** — Windows Credential Manager через крейт `keyring` (enterprise standard, как Raycast/macOS Keychain, gh CLI). НЕ в `dictation-config.json`.
- **DNS для AI HTTP** — настраиваемый DoH resolver (Cloudflare / Google / custom) через `hickory-resolver` в `reqwest::Client::dns_resolver`. Scope: только dictation HTTP. РФ Groq блок — преимущественно DNS poisoning, DoH покрывает 80%.
- **Trigger mode** — только Toggle в Phase 1. Push-to-talk требует low-level hook (`globalShortcut` не даёт keyup) — Phase 1.5.
- **Inject** — `enigo` Ctrl+V симуляция + `arboard` clipboard save/restore + Win32 `SetForegroundWindow(prev_hwnd)` с `sleep 80ms` до и после Ctrl+V. Race с slow paste'ом → restore старого клипа поверх transcript'а — закрыто sleep'ами.
- **Batch submit, не streaming chunks** — Groq endpoint не принимает streaming audio (это transcription, не realtime ASR). Усложнять state machine ради этого не имеет смысла.

Roadmap:

- Phase 2 — локальные модели + download manager + AudioWorklet streaming.
- Phase 3 — LLM post-processing транскрипта (cleanup, пунктуация, стиль под контекст).
- ~~Phase 1.5~~ — push-to-talk через low-level hook + HTTP/SOCKS proxy + custom Whisper prompt — **DONE 2026-05-24**:
  - PTT через Win32 `WH_KEYBOARD_LL` hook в `platform/runtime/src/dictation/hotkey_hook.rs`. Hook парсит accelerator → `Matcher`, emit'ит broadcast `dictation_ptt_trigger { phase }`. Electron в PTT mode НЕ регистрирует globalShortcut, листает hook events и вызывает `toggleDictation()` на каждое (down → старт, up → отправка).
  - Proxy через `reqwest::Proxy::all(url)` (feature `socks` в reqwest). Ортогонально DoH — можно комбинировать.
  - Custom prompt — поле `transcriptionPrompt` в config'е, проходит в Groq multipart `prompt` field. Подсказка модели для domain-specific терминов и стиля.

### 2026-05-25 — Dictation Phase 2: disk-first pending queue + retry с классификацией

Источник: `concepts/dictation.md` § Resilience + `.agent/tasks/2026-05-25-dictation-resilience/spec.md`.

Phase 1 + 1.5 в продакшене вскрыли три класса проблем: (1) audio loss при network drop — renderer обнулял PCM buffer сразу после `submit_audio`, retry невозможен; (2) Custom DoH silent-fallback'ил на Cloudflare — UI ручка обманывала; (3) `op_test_connectivity` пакует ошибку в JSON без tracing — диагностика недоступна.

Решение — **persist-before-upload + классификация + детальный probe**:

- **`dictation::pending`** — disk-first очередь WAV+JSON в `<dataDir>/dictation/pending/<uuid>.*`. Атомарная запись через `tmp → rename`. GC на app start: 7 дней / 20 items LRU.
- **`dictation::retry`** — `classify(&SubmitError) → FailureKind { Retryable | Fatal { user_msg } }`. Network/429/5xx → retryable с `with_backoff(3, 500ms, 8s)` + jitter ±50%. 401/400/413/AudioDecode/NoApiKey → fatal без retry.
- **`network::validate_custom_doh_url`** — реальный парсинг (схема https, host, port, path, IP-литералы IPv4/IPv6). Невалидный URL → `NetworkError::CustomDohInvalid(msg)`. Backend дублирует фронтовую валидацию (`SecurityTab` @blur).
- **`probe_connectivity`** — пошаговый probe `client_build → dns_resolve → tcp_connect → http_head` с временами по стадиям + первая failing + IP/HTTP status в info. UI рендерит чек-листом с подсказкой для firstFailure.
- **Tracing** — `tracing::{info,warn,error}` во все error paths `dictation/{host,network,retry,pending}.rs`. `RUST_LOG=kepler_backend::dictation=debug` теперь даёт полную картину.

Почему disk-first, а не in-memory queue: backend crash, OS reboot, app kill — всё уносит in-memory. WAV (~1MB / 30s аудио × 20 items = ~20MB max) — дёшево. Прецеденты: Wispr Flow держит 14 дней, но у нас личный лаунчер, не SaaS — 7 дней покрывает 95% («ноут в самолёте на день, дома доехало»).

Почему 3 попытки 500ms/1s/2s + jitter: AWS Builders' Library — thundering herd при восстановлении сети. Jitter ±50%. Суммарно <10s — юзер не успевает начать новую диктовку. На retryable исчерпан → pending остаётся на диске; юзер вручную «Повторить» из Settings → Диктация → Очередь.

Sub-решения:

- **WS endpoints**: `dictation.list_pending` / `retry { uuid }` / `discard { uuid }` / `retry_all`.
- **Pill UX**: на retryable error pill остаётся открытым с retry-кнопкой (↻). Fatal error → auto-close 1.2s как раньше. Pending state — для будущего event-stream подхода (сейчас inline-await).
- **`submit_audio` API**: всегда возвращает `{ uuid, state, error, canRetry }` (OK даже при ошибке) — pill получает uuid для retry.
- **Backwards compat**: `groq::transcribe` теперь принимает endpoint аргументом — позволяет httpmock в тестах, прод использует `groq::GROQ_ENDPOINT`.

Roadmap дальше (Phase 3+):

- Локальные модели (whisper.cpp / Parakeet) с auto-fallback при offline — отдельный proof-loop.
- Backend-pushed state events → renderer без polling (для live «attempts N/3» на pill во время retry).
- Persistent история **успешных** транскриптов через ARK objects (история ≠ recovery-queue).

### 2026-06-19 — Dictation local models + AI settings

Источник: `concepts/dictation.md` + `.agent/tasks/2026-06-19-local-dictation-ai-settings/spec.md`.

После проверки Handy workflow принято решение добавить локальный STT без ручного выбора файлов пользователем:

- Settings → AI отвечает за online/local AI: Groq status/test/model, local model catalogue, download progress, storage summary.
- Settings → Диктация отвечает за поведение диктации: hotkey/language/inject mode/provider (`online` или `local`).
- Local model storage — только `<dataDir>/dictation/models/`. Это переживает app update/reinstall и изолируется по instance/data dir; Settings → About показывает, сколько занимают DB/backups/models.
- Download manager делает resumable download и показывает progress. Long-running download не должен идти через короткий 30s Ark IPC timeout.
- Local STT adapter больше не требует packaged `Kosmos Local STT` sidecar. Backend запускает выбранный `whisper-cli` / `whisper-server` из лениво скачанного runtime; UI не знает, какой native executable/engine внутри, и выбирает только provider/model.
- Groq остаётся online fallback: тот же pending/retry/network/proxy путь, API key по-прежнему только в Credential Manager / Keychain.

Причина: желаемый UX такой же, как в Handy — выбрать модель из списка, скачать, использовать. Ручные пути к `.bin`, `whisper-cli.exe` или прямой `whisper-server.exe` оставлены только как debug/advanced escape hatch, не как основной workflow.

## Шаблон для нового решения

Все новые архитектурные/безопасностные решения **обязаны** попадать сюда. Минимальный шаблон ADR:

```markdown
# <Краткое название>

**Дата:** YYYY-MM-DD
**Статус:** Accepted / Proposed / Superseded by <ADR>

## Контекст

Что было неудобно / не работало / противоречиво.

## Решение

Что выбрали и почему именно так.

## Последствия

- Что улучшится.
- Что усложнится.
- Что точно нельзя делать после этого решения.

## Альтернативы, которые отбросили

- Альтернатива A — почему не подошла.
- Альтернатива B — почему не подошла.
```

Сохраняй в `docs/<KEBAB-CASE-NAME>.md` и добавляй ссылку сюда.
