# Dictation (STT)

Источник: `platform/runtime/src/dictation/` + `platform/desktop/electron/dictation-pill.ts` + `platform/desktop/src/views/DictationPillView.vue`.

## За 30 секунд

Dictation — голосовой ввод по образцу [Raycast Dictation](https://manual.raycast.com/ai/dictation). Global hotkey → плавающая **pill** (120×36, frameless, alwaysOnTop, `focusable: false`) → запись микрофона через Web Audio API → транскрипция через выбранный provider (**Groq Cloud** или локальный **Kosmos Local STT** sidecar) → **auto-paste** транскрипта в активное окно через симуляцию Ctrl+V.

По умолчанию можно работать через Groq. Локальный режим настраивается в Settings → AI: пользователь выбирает модель, жмёт «Скачать», backend кладёт assets в shared `<APPDATA>/Kosmos/models|tools/dictation/` (или `KOSMOS_LOCAL_STT_DIR`) и запускает отдельный `Kosmos Local STT` sidecar без ручного указания путей в обычном workflow.

## Поток

```
Hotkey press
   │
   ▼  (main → backend)
dictation.capture_foreground_window  →  Win32 GetForegroundWindow → HWND saved
   │
   ▼
dictation.start_recording  →  state: Idle → Recording
   │
   ▼
showInactive() pill window  +  IPC kepler:dictation:command { kind: "start" }
   │
   ▼  (pill renderer)
getUserMedia + AudioContext + ScriptProcessorNode → Int16 PCM accumulator
   │
   ▼  Hotkey press снова  (toggle mode)
IPC kepler:dictation:command { kind: "stop" }
   │
   ▼
WAV encode (16-bit PCM, 16kHz, mono) → base64
   │
   ▼  (renderer → backend через ws ark.request)
dictation.submit_audio { audioB64 }  →  state: Recording → Transcribing
   │
   ▼
backend split на ≤30s WAV chunks для long-form audio
   │
   ▼
selected provider:
  • Groq → POST api.groq.com/openai/v1/audio/transcriptions (reqwest + опциональный DoH/proxy)
  • Local → `Kosmos Local STT` sidecar + downloaded local assets from shared <APPDATA>/Kosmos/models|tools/dictation/
   │
   ▼
broadcast event dictation_transcript { text, language, durationMs }
   │
   ▼  (backend → enigo + arboard)
spawn_blocking inject:
   1. save clipboard
   2. clipboard.set_text(transcript)
   3. SetForegroundWindow(prev_hwnd)
   4. sleep 80ms
   5. enigo Ctrl press → V click → Ctrl release
   6. sleep 80ms  ← КРИТИЧНО, см. forbidden.md
   7. clipboard.set_text(original)  ← restore
   │
   ▼
state: Transcribing → Idle  +  pill.pillFinished() → hide window
```

## Компоненты

| Файл                                                         | Что делает                                                                                                                                                                                                                    |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `platform/runtime/src/dictation/host.rs`                     | State machine `Idle → Recording → Transcribing → Idle\|Error`. Dispatch для `dictation.*` operations, provider selection, local model lifecycle/download orchestration. `broadcast::Sender<Value>` для WS events.             |
| `platform/runtime/src/dictation/groq.rs`                     | Multipart POST к `/openai/v1/audio/transcriptions`. `language="auto"` → пропускаем поле (Whisper auto-detect). Long-form WAV режется на ≤30s chunks и склеивается обратно в один transcript.                                  |
| `platform/runtime/src/dictation/local.rs`                    | Local STT client: owns IPC to `Kosmos Local STT`, forwards status/load_model/preload/transcribe/cancel/unload and normalizes responses for `host.rs`.                                                                         |
| `platform/runtime/src/dictation/local_models.rs`             | Каталог локальных моделей, shared storage path `<APPDATA>/Kosmos/models                                                                                                                                                       | tools/dictation/`или`KOSMOS_LOCAL_STT_DIR`, resumable download, checksum/size metadata и прогресс для Settings → AI. |
| `platform/runtime/src/dictation/inject.rs`                   | `enigo` Ctrl+V симуляция + `arboard` clipboard save/restore + `windows::Win32 GetForegroundWindow/SetForegroundWindow` для возврата фокуса. Все sleep'ы 80ms.                                                                 |
| `platform/runtime/src/dictation/config.rs`                   | JSON (`<data_dir>/dictation-config.json`) для не-секретов + `keyring` (Windows Credential Manager target `kosmos-kepler` / user `groq-api-key`) для API ключа.                                                                |
| `platform/runtime/src/dictation/network.rs`                  | Фабрика `reqwest::Client` с переключаемым DNS resolver'ом (System / Cloudflare DoH / Google DoH / custom DoH URL) через `hickory-resolver`. Scope: только AI HTTP.                                                            |
| `platform/desktop/electron/dictation-pill.ts`                | BrowserWindow lifecycle (`focusable: false`, alwaysOnTop, transparent), global hotkey регистрация, IPC `kepler:dictation:toggle/cancel/pill-finished/command`. Hotkey перерегистрируется на event `dictation_config_changed`. |
| `platform/desktop/src/views/DictationPillView.vue`           | Audio capture (Web Audio API), 16-bar pill states (idle/recording/transcribing/waiting/error), timer, WAV encode, base64, отправка через `window.kepler.ark.request("dictation.submit_audio", ...)`.                          |
| `platform/desktop/src/views/settings/tabs/AISettingsTab.vue` | UI для online/local AI: Groq enable/model/status/test, local model catalogue/download progress, storage summary.                                                                                                              |
| `platform/desktop/src/views/settings/tabs/DictationTab.vue`  | UI настроек диктации: hotkey / язык / inject mode / provider selection / retry queue.                                                                                                                                         |

## Безопасность ключа

API key Groq хранится в нативном secret store через крейт `keyring` (target `kosmos-kepler`, user `groq-api-key`): **Windows Credential Manager** (feature `windows-native`) и **macOS Keychain** (feature `apple-native`). Это enterprise-стандарт (так делают Raycast на macOS Keychain, gh CLI, git-credential-manager).

> ⚠️ keyring без backend-feature для текущей платформы молча использует **mock store** (запись «успешна», но ключ не персистит между процессами). На macOS это давало `submit_audio: API key не задан` после ввода ключа — оба feature должны быть в `platform/runtime/Cargo.toml`.

- Ключ **никогда** не попадает в `dictation-config.json`.
- Не утекает через `db_backup` (`backups/` не копирует Credential Manager).
- Не утекает через случайный коммит / crash dump / screen sharing config'а.

Удаление через `dictation.clear_api_key` или вручную через `cmdkey /delete:kosmos-kepler`.

## DNS режимы (РФ доступ)

Groq.com в РФ блокируется в основном через DNS poisoning. Решение Phase 1 — выбор DoH resolver'а в Settings → Безопасность:

- **Системный** (default) — стандартный resolver ОС.
- **Cloudflare DoH (1.1.1.1)** — `ResolverConfig::cloudflare_https()` из `hickory-resolver`.
- **Google DoH (8.8.8.8)** — `ResolverConfig::google_https()`.
- **Custom DoH URL** — Phase 1 fallback на Cloudflare; полный parse custom URL → ServerName + IPs — Phase 1.5.

**Scope: только dictation HTTP клиент.** Sync / RAWG / прочее — системный DNS как раньше (см. `forbidden.md` → Dictation). Spec расширения на другие AI-провайдеры в Phase 2.

DoH не лечит SNI / IP блокировку — для таких случаев roadmap'ом идёт HTTP/SOCKS proxy config.

## State machine (host.rs)

```
   ┌──────┐ start_recording   ┌──────────┐ submit_audio  ┌─────────────┐
   │ Idle ├──────────────────►│Recording ├──────────────►│ Transcribing │
   └──┬───┘                   └────┬─────┘               └──────┬──────┘
      │                            │ cancel                     │
      │                            ▼                            │ on success
      │                         (Idle)                          │ inject + emit
      │                                                         │ event →
      └◄────────────────────────────────────────────────────────┘
                              on error → Error → user clears → Idle
```

Wire events (flat JSON, без nested kind/payload):

```json
{ "event": "dictation_state_changed", "state": "recording", "error": null }
{ "event": "dictation_transcript", "text": "...", "language": "ru", "durationMs": 1234 }
{ "event": "dictation_config_changed" }
```

Operations см. в `host.rs::handle_dictation_op`.

## Push-to-talk (Phase 1.5)

`globalShortcut.register` шлёт только key-down event'ы — для hold-to-record этого мало. Решение: Win32 low-level keyboard hook (`WH_KEYBOARD_LL`) в выделенном OS-потоке (`platform/runtime/src/dictation/hotkey_hook.rs`). Hook парсит accelerator из config'а в `Matcher { vk, ctrl, shift, alt }`, на каждое key event'е проверяет vk + модификаторы через `GetAsyncKeyState`, emit'ит broadcast `dictation_ptt_trigger { phase: "down" | "up" }`.

Electron подписывается на этот event и вызывает `toggleDictation()` — те же два вызова что юзер делал бы в Toggle mode (первый запускает, второй отправляет). В PTT-режиме `globalShortcut` НЕ регистрируется (см. `applyHotkeyForMode` в `dictation-pill.ts`).

Hook callback должен возвращаться очень быстро (Windows блокирует клавиатурный ввод иначе) — только snapshot lock + non-blocking broadcast send. Auto-repeat от удержания дедупится через `pressed` flag.

PTT — Windows-only. На non-Windows hook стаб'нут, доступен только Toggle.

## HTTP / SOCKS proxy (Phase 1.5)

Когда DoH недостаточно (SNI inspection / TCP block), доступен HTTP/SOCKS proxy в Settings → Безопасность. Поле `httpProxy` в `dictation-config.json`, прокидывается в `network::build_client` как `reqwest::Proxy::all(url)`. Поддерживается `http://`, `https://`, `socks5://`. Применяется только к dictation HTTP клиенту (см. forbidden.md).

DoH и proxy ортогональны и могут комбинироваться: DoH резолвит host, proxy туннелирует TCP-соединение.

## Контекстный prompt

Whisper API принимает поле `prompt` — подсказка для domain-терминов / стиля. **UI его не показывает** — используется захардкоженный `groq.rs::HARDCODED_PROMPT`. Причины:

- Лимит prompt'а — 224 токена. Длиннее обрежется, и длинный prompt сам начинает галлюцинироваться в выход (Whisper копирует фразы из него в текст).
- Промпт в стиле «ты транскрибатор, делай то-то» Whisper копирует в выход. Только пример речи + список domain-терминов.
- Промпт на английском для русского аудио не передаёт стиль.

Если `transcriptionPrompt` непустой в JSON-конфиге — он перекрывает hardcoded (configurable hook для тестов / экспериментов).

## Anti-hallucination

В Groq multipart передаются (`groq.rs::transcribe`):

- `language=<ru|en|…>` — самый сильный антигаллюцинационный механизм. Без явного языка Whisper «прыгает» на другой и фантазирует. `auto` пропускает поле, но **не рекомендуется** — снижает качество.
- `temperature=0` — детерминированный декодинг. Default Groq использует динамическую температуру → повторы и «YouTube-loop'ы» («Спасибо за просмотр», «Подписывайтесь на канал»).
- `response_format=verbose_json` — даёт `segments[]` с `no_speech_prob` и `avg_logprob` для каждого фрагмента. `groq::filter_segments` отсеивает:
  - `no_speech_prob > 0.6` — на тишинах Whisper выдаёт YouTube-фразы (артефакт обучения на YouTube).
  - `avg_logprob < -1.0` — низкая уверенность модели, типично галлюцинация.

Если фильтр выкинул **всё** — возвращаем пустую строку, а не raw `text` (который содержал бы те самые галлюцинации).

Не реализовано (Phase 2+):

- **Silero-VAD предобработка** перед отправкой в Groq (резка длинных тишин). Снизит количество no-speech сегментов в принципе.
- Учёт `compression_ratio_threshold > 2.4` (классический Whisper-индикатор повторов).

## Long-form chunking

Groq/Whisper file-based STT принимает длинные файлы, но long-form качество и полнота
лучше, когда аудио подаётся сегментами около 30 секунд. Поэтому backend хранит
исходный WAV в pending как один файл, но перед HTTP отправкой парсит наш PCM WAV
и режет `data` chunk на ≤30s WAV-файлы. Каждый chunk отправляется в тот же
`/audio/transcriptions` endpoint с теми же language/model/prompt настройками,
а результаты склеиваются пробелом в исходном порядке.

Если WAV не похож на ожидаемый 16-bit PCM формат, backend не пытается угадать
структуру и отправляет файл одним запросом — это fallback для будущих источников
аудио, но нормальный renderer path всегда генерирует поддерживаемый PCM WAV.

## Выбор provider / модели

Settings → Диктация выбирает provider: `online` (Groq) или `local` (Kosmos Local STT sidecar). Settings → AI управляет конкретными моделями и подготовкой локального runtime.

- **Groq** — online fallback/default для машин без скачанной модели; API key хранится только в Credential Manager / Keychain.
- **Local STT sidecar** — offline path. Модель скачивается из каталога в shared `<APPDATA>/Kosmos/models|tools/dictation/` или `KOSMOS_LOCAL_STT_DIR`; config хранит model id/path metadata, но не требует от пользователя ручного полного пути.
- **Storage** — модели лежат в user data dir Kosmos и переживают update/reinstall приложения. При обычном uninstall ОС может оставить data dir; Settings → About показывает storage summary, чтобы пользователь видел, сколько занимают DB/backups/models.

## Local STT sidecar

Локальная диктация разделена на два уровня ответственности:

- `platform/runtime` владеет dictation state, pending queue, retries, config и injection.
- `Kosmos Local STT` sidecar владеет native STT engine lifecycle: загрузкой модели, выбором backend/accelerator, preload/transcribe/cancel/unload и teardown после idle.
- Local backend не выбирается пользователем: prod policy берёт `faster-whisper` на Windows и `whisper.cpp` на macOS. Backend/tool ставится только в операции «Скачать модель»: Windows создаёт managed Python venv в shared `<APPDATA>/Kosmos/tools/dictation/faster-whisper/.venv/`, ставит `faster-whisper`, готовит отдельный CTranslate2/HF cache рядом и сохраняет backend model id (`large-v3-turbo` и т.п.). `KOSMOS_FASTER_WHISPER_PYTHON` остаётся escape hatch для явного Python runtime. Если setup падает, запрос возвращает controlled local STT error, pending WAV не удаляется, и backend не переключается на другой движок.
- Managed whisper.cpp по умолчанию ставит CPU build. CUDA build скачивается только при `KOSMOS_DICTATION_ENABLE_CUDA=1`; уже установленный CUDA `whisper-cli.exe` остаётся fallback'ом.
- Managed faster-whisper ставит только `faster-whisper` и CTranslate2/HF cache. CUDA wheels (`nvidia-cublas-cu12` / `nvidia-cudnn-cu12`) не ставятся автоматически: они раздувают локальные инструменты на гигабайты. Для GPU-экспериментов используйте `KOSMOS_FASTER_WHISPER_DLL_DIRS` / `KOSMOS_FASTER_WHISPER_DLL_DIR` с явным каталогом CUDA DLL.
- Desktop IPC для `dictation.submit_audio` ограничен 60 секундами. Внутренний Python child у `faster-whisper` имеет отдельный предел 55 секунд (`KOSMOS_FASTER_WHISPER_TIMEOUT_MS`, clamped 5-55s), чтобы зависший backend завершался раньше transport timeout.
- Tech debt: macOS policy уже выбирает `whisper.cpp`, но managed download flow ждёт packaged macOS `whisper.cpp` tool artifact. До добавления артефакта macOS локальный backend остаётся controlled unsupported, без fallback на другой движок.
- Sidecar сам запускает idle watcher: по умолчанию он проверяет бездействие каждые 10 секунд и выгружает модель после `idle_unload_after_ms`, не дожидаясь следующего `status`/`transcribe` запроса. При переключении dictation provider с `local` на online provider host отправляет sidecar `unload`.

Host общается с sidecar только через локальный IPC с шестью операциями:

- `status`
- `load_model`
- `preload`
- `transcribe`
- `cancel`
- `unload`

Смысл boundary простой: если native engine падает, падает только sidecar. `platform/runtime` получает controlled local STT failure, не удаляет pending WAV и может поднять новый sidecar на следующем local request. Это сохраняет crash isolation между product runtime и нативным whisper engine.

Debug escape hatch остаётся только для диагностики: прямой запуск `whisper-cli.exe` допустим вручную, но не является normal product path.

## Post-mortems (0.3.0 → 0.3.1)

После первого реального использования всплыли 4 серьёзных бага. Фиксы в 0.3.1, оставляем как permanent reference чтобы не наступить на те же грабли.

### 1. `TryFromIntError(())` при inject'е транскрипта

**Симптом:** transcription приходит, но Ctrl+V не симулируется. В логах `enigo input: you tried to simulate invalid input: (key state could not be converted to u32)`.

**Причина:** в `inject.rs` мы дёргали `enigo.key(Key::Unicode('v'), Direction::Click)`. `Key::Unicode` на Windows шлёт символ через **VK_PACKET** (Unicode injection channel) — это литеральный ввод символа, **не** virtual-key. На VK_PACKET модификаторы (Ctrl, Shift) **не работают** как часть shortcut'а — Windows их игнорирует. Сам enigo при попытке упаковать keystate в `u32` падал с `TryFromIntError`.

**Фикс:** заменили enigo на нативный `windows::Win32::UI::Input::KeyboardAndMouse::SendInput` с `VK_CONTROL` + `VK_V` (0x56). Это стандартный virtual-key канал, на котором модификаторы работают как shortcut. См. `inject.rs::send_ctrl_v`. Зависимость `enigo` удалена.

**Урок:** для accelerator'ов / shortcut'ов на Windows — **всегда** virtual-key channel, **никогда** Unicode channel. Unicode только для непосредственного ввода текста (если бы мы хотели вместо clipboard набивать буквы вручную — но это медленно и зависит от раскладки).

### 2. Pill race condition — первое нажатие hotkey'я не запускало запись

**Симптом:** «нажимаю на хоткей и виджет появляется, но запись не идёт». Второе нажатие → ничего. Третье → запись стартует.

**Причина:** `toggleDictation()` создавал pill window лениво (`ensureWindow → createPill → loadURL`), сразу `showInactive()` + `webContents.send("kepler:dictation:command", {kind: "start"})`. На холодном старте renderer ещё не успевал смонтировать Vue компонент → `onMounted` не отработал → подписка на `onCommand` ещё не активна. Команда `start` уходила в пустоту.

**Фикс:** добавили `pillReady: Promise<void>`, резолвится по `did-finish-load` + 50ms tick (Vue mount). `sendPillCommand` ждёт `pillReady` перед `webContents.send`. После 0.3.1 ещё добавили **idle warmup** (BrowserWindow создаётся через 3s после старта Kepler) — `pillReady` к моменту первого hotkey уже резолвлен.

**Урок:** Electron `webContents.send` не имеет flow-control. Если renderer не подписан — событие теряется. Для lazy-created окон **обязательно** ждать `did-finish-load` + один tick фреймворка перед первой отправкой.

### 3. Модификаторы терялись при capture системных hotkey'ев (Win+H → H)

**Симптом:** в Settings → Диктация → Горячая клавиша назначаешь `Win+H`, но прилетает только `H` без модификатора.

**Причина:** capture-handler в hook'е intercept'ил **modifier-нажатия** (Win/Ctrl/Shift/Alt) с `return LRESULT(1)`. Windows **не успевала зарегистрировать их state** — потому что keyboard hook chain отрабатывает ДО того как ОС обновит внутреннее состояние. Когда юзер потом дожимал `H`, мы проверяли модификаторы через `GetAsyncKeyState(VK_LWIN)` — он возвращал «не нажато», потому что мы же сами перехватили само Win-down событие.

**Фикс:** modifier-нажатия теперь **пропускаются** через `CallNextHookEx` (без intercept). Это безопасно — `RegisterHotKey` системных shortcut'ов (Win+H) опирается на `WM_HOTKEY`, который генерируется **после** всего hook chain'а. Если мы intercept'им именно final-клавишу (H), WM_HOTKEY для Win+H не генерируется → Voice Typing не запустится.

**Урок:** `GetAsyncKeyState` читает **системный** state, не наше восприятие событий. Если мы перехватили событие до системы — для системы клавиша не нажималась.

### 4. Start menu открывался после Win+H intercept'а

**Симптом:** Win+H успешно intercept'ится, dictation запускается — но при отпускании Win открывается Start menu.

**Причина:** Windows открывает Start menu когда Win-down → Win-up прошли **без других клавиш между ними**. Мы intercept'или H (`LRESULT 1`) → для ОС последовательность выглядит как «голое нажатие Win».

**Фикс:** стандартный приём AutoHotKey / PowerToys — функция `swallow_win_shortcut_if_active()` в hook'е. После успешного intercept'а проверяем, зажат ли Win, и если да — посылаем dummy `SendInput` с `VK_RESERVED (0xFF)` (down+up). Это безымянный virtual-key без визуального эффекта, **но** Windows регистрирует его как «клавиша между Win-down и Win-up» → считает Win использованным как modifier → Start menu trigger подавляется.

**Урок:** Windows shell отслеживает Win key как modifier через **state machine** (Win pressed → other key seen?), не через message flow. Перехват single key не нарушает этот state machine — нужно явно его «накормить» dummy событием.

## Known issues / tech debt

Накапливаем здесь то, что обнаружено в продакшене, но не успели или не смогли починить с первой итерации. При следующем proof loop'е по диктации — раскопать и закрыть.

### `Win+Ctrl+V` (audio output picker) иногда открывается при Win+H (2026-05-25)

**Симптом:** при использовании `Win+H` как hotkey'я диктации, **иногда** при отпускании `H` (когда `Win` всё ещё зажат) Windows открывает popup аудио-выхода («Аудиовыход», Win+Ctrl+V).

**Что уже пробовали:**

1. Defer `swallow_win_shortcut_if_active` SendInput в spawned thread (был race с hook timeout / рекурсия через свою же hook chain). Помогло частично.
2. Сменили dummy VK с `VK_RESERVED (0xFF)` на `VK_NONAME (0xFC)` — официально reserved by Microsoft. `0xFF` undefined, Windows иногда интерпретировал как edge-case с `Win+Ctrl+V` mapping. После смены ещё пробивается.

**Возможные направления:**

- Использовать `KEYEVENTF_SCANCODE` со scan code 0x00 вместо virtual key — обходит VK mapping вообще.
- Делать два разных swallow: на keydown (как сейчас) **и** на keyup непосредственно перед reлизом Win — чтобы Windows точно увидела «modifier consumed».
- Посмотреть как именно делает [PowerToys Keyboard Manager](https://github.com/microsoft/PowerToys) — у них работает 100%.
- Альтернатива: рекомендовать пользователям не Win+H, а Ctrl+Alt+Space / другую non-system комбинацию — но это компромисс с UX.

**Workaround для пользователя:** отпускать `Win` раньше `H` (или одновременно).

---

## Resilience (Phase 2)

Phase 2 закрыло 3 класса проблем, выявленных в продакшене Phase 1+1.5:
audio loss при сетевом сбое, неработающий Custom DoH, плоский «не удалось» в test_connectivity.

### Disk-first очередь pending

WAV пишется на диск **до** HTTP-запроса. Crash backend / kill app / network drop — аудио переживает, доступно для retry.

```
<dataDir>/dictation/pending/
  ├── <uuid>.wav   — сырое аудио (16kHz/16bit/mono)
  └── <uuid>.json  — { uuid, createdAt, attempts, lastError, opts, durationSec, wavBytes }
```

Атомарность через `.tmp → rename`. GC на app start: удалить items старше 7 дней ИЛИ свыше 20 штук (LRU по createdAt). См. `pending.rs`.

### Retry + классификация ошибок

`retry::classify(&SubmitError) -> FailureKind` различает:

- **Retryable** — сетевые (`reqwest::Error` любой), Groq `429` / `5xx`. Срабатывает `retry::with_backoff(3, 500ms, 8s)` с jitter ±50%.
- **Fatal** — Groq `401` («Неверный API key»), `413` («Аудио слишком длинное»), `400` («Groq отклонил»), `AudioDecode`, `NoApiKey`. Без retry, pill сразу в Error state.

При исчерпании retry pending остаётся на диске — юзер вручную нажимает «Повторить» из Settings → Диктация → Очередь диктовок или из retry-кнопки на pill.

### Custom DoH — реальная реализация

Phase 1 silent-fallback'ил на Cloudflare. Phase 2: `network::validate_custom_doh_url` парсит URL (схема, host, port, path, IP-литерал), `build_resolver` собирает `hickory_resolver::NameServerConfig` с `Protocol::Https`. Невалидный URL → `Err(NetworkError::CustomDohInvalid)`, UI показывает причину inline.

### Stage-by-stage диагностика

`dictation.test_connectivity` теперь возвращает:

```jsonc
{
  "ok": false,
  "totalMs": 881,
  "firstFailure": "tcp_connect",
  "stages": [
    { "name": "client_build", "ok": true, "ms": 12 },
    { "name": "dns_resolve", "ok": true, "ms": 84, "ip": "104.18.32.115" },
    { "name": "tcp_connect", "ok": false, "ms": 5000, "error": "TCP connect timeout (5s)" },
  ],
}
```

UI рендерит чек-листом + подсказку для первой failing стадии («TCP connect отвергнут — возможен IP-блок, попробуйте proxy»).

### WS endpoints (Phase 2 additions)

- `dictation.list_pending` → `{ items: [...] }` для UI очереди.
- `dictation.retry { uuid }` → запуск process_pending в фоне.
- `dictation.discard { uuid }` → удаляет файл, сбрасывает state если был активен.
- `dictation.retry_all` → запуск всех pending параллельно.

### Tracing

`tracing::{info,warn,error}` во всех error path в `dictation/{host,network,retry,pending}.rs`. Включается через `RUST_LOG=kepler_backend::dictation=debug`. Без этого Phase 1 диагностика была невозможна — все ошибки молча уходили в JSON-ответ WS.

---

## Что НЕ делает текущая диктация

- LLM post-processing транскрипта (filler-word cleanup, пунктуация, стиль) — Phase 3.
- DoH для не-AI запросов (sync / RAWG / прочее) — out of scope.
- Linux порт — Win32 `GetForegroundWindow` + `WH_KEYBOARD_LL` специфичны; native helpers есть только под macOS.
- Persistent история транскрипций.
- Product path не зависит от direct `whisper-cli.exe`; это только debug/advanced fallback.

## macOS hotkey capture (adapter)

Назначение хоткея в Settings → Диктация идёт через backend-op `dictation.begin_hotkey_capture` (adapter-метод) одинаково на обеих платформах; «как именно ловим сочетание» инкапсулировано per-OS внутри адаптера, UI платформо-агностичен (безусловный `external-capture`).

- **Windows** — `host.rs::op_begin_hotkey_capture` → low-level hook (`hotkey_hook::set_capture_mode`), эмитит `dictation_capture_key { vk, ctrl, shift, alt, win }`.
- **macOS** — Swift helper `capture-hotkey` (`platform/runtime/native/macos/capture-hotkey.swift`): `CGEventTap` без фильтра по keyCode, ловит первый `keyDown` с ≥1 квалифицирующим модификатором (cmd/ctrl/alt/shift). `macos_native::begin_capture` / `end_capture` читают helper, **резолвят mac `keyCode` → accelerator-строку в Rust** (`mac_key_name` — обратный маппинг к `mac_key_code`; cmd→`Super`) и эмитят `dictation_capture_key { accelerator }`; `Escape` → `dictation_capture_cancelled`.

::: tip Почему accelerator из Rust, а не `vk`
macOS `CGKeyCode` ≠ Windows VK: фронтовый `vkToKeyName` ждёт Windows-коды. Поэтому строку собирает адаптер (`build_accelerator`), а `useDictationConfig.onDictationCaptureStart` предпочитает `e.accelerator` если он есть — ветка платформо-агностична, leak в UI не возвращается.
:::

История долга (variation point протекал в UI через `:external-capture="isWindows"`, закрыт 2026-06-07) — `.agent/tasks/2026-06-07-macos-hotkey-capture-adapter/spec.md`.

## ADR

См. `reference/decisions.md` § 2026-05-24 — «Phase 1 STT — облачный Groq, не локальный whisper.cpp» и § 2026-06-19 — «Dictation local models + AI settings».
