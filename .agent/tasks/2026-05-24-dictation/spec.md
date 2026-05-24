# 2026-05-24 dictation (Phase 1, Groq cloud)

## Context

В Kepler нет голосового ввода. Цель Phase 1 — Raycast-style диктация:
глобальный hotkey → плавающая "Dictation Pill" над активным окном → запись
аудио → транскрипция в Groq Cloud (Whisper-large-v3-turbo) → авто-вставка в
активное приложение через симуляцию Ctrl+V.

Прецедент open-source — [cjpais/Handy](https://github.com/cjpais/Handy)
(Tauri + cpal + whisper-rs + enigo). Мы заимствуем подход (Rust backend +
clipboard restore-trick), но Phase 1 — облачная транскрипция, локальные
модели (whisper.cpp, Parakeet V3) — отдельный proof-loop в Phase 2.

Дополнительная цель — фича должна работать в РФ без VPN. Большинство
блокировок Groq на сетевом уровне — DNS poisoning. Решение — настраиваемый
DoH resolver (Cloudflare 1.1.1.1 / Google 8.8.8.8 / custom DoH URL) в
HTTP-клиенте к AI-провайдерам. SNI/IP-блок DoH не лечит — для этого
roadmap'нем proxy support отдельно.

## Scope

### Backend — `services/kepler-backend/src/dictation/`

1. **`mod.rs`** — публичные types (`DictationState`, `DictationEvent`,
   `DictationConfig`, `NetworkProfile`, `InjectMode`, `TriggerMode`).
2. **`host.rs`** — state machine: `Idle → Recording → Transcribing →
Idle | Error`. `Arc<Mutex<State>>` с poison recovery (см.
   `concepts/db-resilience.md`). `broadcast::Sender<DictationEvent>` для
   подписки WS-клиентов. Шаблон — `pomodoro_host.rs:50-380`. Хранит:
   - `state: DictationState`
   - `current_audio_buf: Option<Vec<u8>>` (batch mode)
   - `prev_foreground_hwnd: Option<isize>` (для focus restore)
3. **`groq.rs`** — клиент к Groq API.
   - Endpoint: `POST https://api.groq.com/openai/v1/audio/transcriptions`
   - Multipart body: `file=<wav_bytes>`, `model=whisper-large-v3-turbo`,
     `language=<config.language>`, `response_format=verbose_json`
   - Auth: `Authorization: Bearer <api_key из keyring>`
   - Timeout: 60s
   - Через `reqwest::Client`, собранный фабрикой `network::build_client`.
4. **`inject.rs`** — `enigo::Enigo` + `arboard::Clipboard`.
   - `pub fn inject(text: &str, mode: InjectMode, prev_hwnd: Option<HWND>)`
   - Поток (AutoPaste): 1. Сохранить текущий clipboard (text/HTML/files если возможно). 2. Записать transcript в clipboard. 3. Если `prev_hwnd` известен — `SetForegroundWindow(prev_hwnd)`,
     затем `tokio::time::sleep(80ms)` (Windows должна вернуть фокус). 4. `enigo.key(Key::Control, Press); enigo.key(Key::V, Click);
enigo.key(Key::Control, Release);` 5. `tokio::time::sleep(80ms)` (даём ОС прочитать буфер). 6. Восстановить оригинальный clipboard.
   - Поток (ClipboardOnly): только шаг 2. Юзер сам жмёт Ctrl+V.
5. **`config.rs`** — JSON для не-секретов + keyring для API key.
   - Path: `<instance.dataDir>/dictation-config.json` (через
     `resolveInstance` — slot-based изоляция prod/dev/test).
   - Keyring: `keyring::Entry::new("kosmos-kepler",
"groq-api-key")` → Windows Credential Manager target
     `kosmos-kepler` / username `groq-api-key`.
   - Структура JSON:
     ```json
     {
       "hotkey": "Ctrl+Shift+;",
       "trigger_mode": "push_to_talk",
       "language": "ru",
       "inject_mode": "auto_paste",
       "dns_profile": "system",
       "custom_doh_url": null,
       "provider": "groq",
       "model": "whisper-large-v3-turbo"
     }
     ```
6. **`network.rs`** — фабрика `reqwest::Client` с переключаемым
   DNS resolver'ом.
   - `pub enum NetworkProfile { System, CloudflareDoh, GoogleDoh,
CustomDoh(String) }`
   - `pub fn build_client(profile: &NetworkProfile) ->
reqwest::Result<reqwest::Client>`
   - Для не-`System` строит `hickory_resolver::TokioResolver` с
     `Protocol::Https` config + `NameServerConfigGroup::cloudflare_https()`
     / `google_https()` / custom (parse URL → ip + ServerName).
   - Wraps в `reqwest::dns::Resolve`.
   - Generic API — переиспользуется для будущих AI провайдеров.

7. **WS endpoints** в `ws_server.rs` под namespace `dictation.*`:
   - `dictation.start_recording { trigger_mode }` → возвращает
     `{ ok }`, фронт-сторона уже владеет audio capture'ом.
   - `dictation.capture_foreground_window` → backend вызывает
     Win32 `GetForegroundWindow()` (через `windows` crate), сохраняет
     HWND. Вызывается ДО показа pill-окна.
   - `dictation.submit_audio { audio_b64, format, sample_rate }` →
     transition Recording→Transcribing → POST в Groq → emit
     `dictation_transcript` → если `inject_mode=auto_paste`, сразу
     вызвать `inject::inject(...)` → transition Idle.
   - `dictation.cancel` → drop buffer, Idle.
   - `dictation.get_state` → `{ state, has_api_key, ... }`.
   - `dictation.set_api_key { key }` → keyring write.
   - `dictation.clear_api_key` → keyring delete.
   - `dictation.update_config { ...patch }` → merge + save JSON +
     emit `dictation_config_changed` (фронт re-фетчит).
   - `dictation.get_config` → возвращает config без секрета, +
     `has_api_key: bool`.
   - `dictation.test_connectivity` → выполняет `HEAD api.groq.com`
     через выбранный DNS-профиль, возвращает `{ ok, resolved_ip?,
latency_ms? }`. Для UI кнопки.

8. **События (broadcast)**:
   - `dictation_state_changed { state, error? }`
   - `dictation_transcript { text, language, duration_ms }`
   - `dictation_config_changed`

### Cargo deps (`services/kepler-backend/Cargo.toml`)

- `enigo = "0.2"` (cross-platform input simulation, Windows SendInput)
- `arboard = "3"` (clipboard read/write с поддержкой HTML/image)
- `keyring = "3"` (Credential Manager)
- `hickory-resolver = { version = "0.24", features =
["dns-over-https-rustls"] }`

`windows` crate — уже есть (используется в focus mode helper'е), нужны
features `Win32_UI_WindowsAndMessaging` + `Win32_Foundation`.

`reqwest` — уже есть.

### Electron — `shell/electron/`

1. **`dictation-pill.ts`** — overlay window. Шаблон —
   `focus-widget.ts:1-533`.
   - `frame: false, alwaysOnTop: true, transparent: true,
skipTaskbar: true, resizable: false, focusable: false`
   - Размер 320×56. Position — центр сверху над cursor monitor'ом.
   - `show: !headless && !testMode` (см. `focus-widget.ts:201-202`).
   - НЕ забирает фокус (`focusable: false` + `show()` без `focus()`).
   - IPC namespace `kepler:dictation:*`:
     - `kepler:dictation:show` (main → renderer) — pill отрисовывает себя,
       начинает getUserMedia.
     - `kepler:dictation:hide` (main → renderer) — освобождает stream.
     - `kepler:dictation:audio-ready` (renderer → main) — Int16 PCM
       base64 готов, main отправляет в backend.
     - `kepler:dictation:state-update` (broadcast).
2. **`dictation-hotkey.ts`** — регистрация globalShortcut с
   поддержкой push-to-talk и toggle. Re-register по событию
   `dictation_config_changed`.
   - Для push-to-talk: нативный globalShortcut не даёт keyup. Решение —
     hot-trigger на key press запускает recording, окно закрывается по
     IPC-сигналу от renderer'а (renderer слушает `keyup` на любом
     элементе fokus'е НЕТ — pill `focusable: false`). Workaround:
     используем глобальный `iohook`-стиль через **отдельный low-level
     hook** — НЕ внедряем, для MVP оставляем **toggle** (press once
     start, press again stop). Push-to-talk пометим TODO Phase 1.5
     (требует `node-global-key-listener` или Rust-side hook через
     `windows-rs SetWindowsHookExW`).
3. **Обновление `main.ts`** (line 1557-1595, рядом с launcher
   globalShortcut) — импорт + регистрация dictation hotkey,
   перерегистрация через callback.
4. **Обновление `preload.ts`** — `window.kepler.dictation` namespace
   с методами: `start`, `cancel`, `getState`, `getConfig`,
   `updateConfig`, `setApiKey`, `clearApiKey`, `testConnectivity`,
   `onStateChange(cb)`, `onTranscript(cb)`, `submitAudio(b64)`.

### Renderer — `shell/src/`

1. **`shell/src/views/DictationPill.vue`** — pill content.
   - Web Audio API: `getUserMedia({ audio: { sampleRate: 16000,
channelCount: 1, echoCancellation: true, noiseSuppression: true } })`.
   - `AudioContext` + `AnalyserNode` → локальная waveform (canvas,
     RMS/FFT). Никакого стриминга на бэк — waveform только для UI.
   - PCM accumulation: `ScriptProcessorNode` (MVP, deprecated но
     работает) → Float32 → Int16 → push в buffer.
   - Timer (mm:ss), max 60 sec (Groq limit для batch, после
     обрезаем).
   - На stop: encode buffer в WAV (16-bit PCM, 16kHz, mono) → base64
     → `window.kepler.dictation.submitAudio(b64)`.
   - На cancel: drop buffer.
2. **`shell/src/views/settings/SecurityPage.vue`** — новая main-group
   страница "Безопасность".
   - Секция "Сеть для AI-провайдеров":
     - Radio: Системный DNS / Cloudflare DoH (1.1.1.1) /
       Google DoH (8.8.8.8) / Custom DoH URL.
     - Input для custom URL (только если выбран custom).
     - Кнопка "Проверить соединение" → `dictation.test_connectivity`.
     - Hint про РФ-доступ к Groq.
   - Архитектурно generic — переиспользуется для будущих AI-провайдеров
     (название поля — "Сеть для AI", не "Сеть для Groq").
3. **`shell/src/views/settings/DictationPage.vue`** — новая
   advanced-group страница "Диктация".
   - Hotkey picker (re-use launcher hotkey UI если есть, иначе
     скопировать).
   - Radio "Режим": Push-to-talk (disabled с tooltip "В разработке") /
     Toggle.
   - Select "Язык": ru / en / auto.
   - Radio "Вставка": Auto-paste / Только в буфер обмена.
   - Поле "Groq API key" (type=password). Placeholder:
     "Сохранено в Windows Credential Manager" если есть. Кнопка
     "Очистить".
   - Ссылка "Получить ключ — console.groq.com".
   - Hint "API key хранится в Windows Credential Manager, не в
     обычном файле конфига."
4. **`shell/src/views/SettingsView.vue:189-300, 1807-1843`** —
   добавить nav items + switch cases.

### Doc updates

- **`docs-site/concepts/dictation.md`** — concept page: архитектура,
  поток, безопасность ключа, DNS-режимы, что НЕ делает (нет
  push-to-talk, нет локальных моделей, нет LLM post-processing).
- **`docs-site/agents/forbidden.md`** — секция "Dictation":
  - ❌ Хранить API key в `dictation-config.json` или любом JSON.
    Только keyring. Reason: ключ не должен утекать через бэкап
    AppData, случайный коммит, crash dump.
  - ❌ `BrowserWindow` для pill вне `dictation-pill.ts`.
  - ❌ Inject без восстановления оригинального clipboard.
  - ❌ Restore clipboard < 50ms после симуляции Ctrl+V (race с
    ОС → вставится старый текст). Минимум 80ms tokio sleep.
  - ❌ Inject без `SetForegroundWindow(prev_hwnd)` + sleep 80ms
    перед симуляцией клавиш (иначе текст улетит в pill или в
    случайное окно).
  - ❌ Streaming audio chunks в Phase 1 (Groq не принимает streaming,
    лишний код). Batch submit только.
  - ❌ `getUserMedia` без graceful permission denied handling
    (показать понятную ошибку, не белый экран).
  - ❌ Hotkey registration без headless guard'а (в тестах
    globalShortcut не регистрируется).
  - ❌ Использовать DoH resolver для не-AI запросов (sync/RAWG/etc)
    в Phase 1. Scope явно ограничен AI-провайдерами.
- **`docs-site/reference/decisions.md`** — ADR:
  «Phase 1 STT — облачный Groq, не локальный whisper.cpp. Причина:
  MVP за разумные часы, локальные модели — отдельный proof-loop в
  Phase 2 (whisper.cpp + Parakeet V3 + download manager + CUDA).»
- После публичных правок: `bun run docs:sync` → `bun run docs:check`.

## Phase 1.5 (done 2026-05-24)

- **Push-to-talk** через Win32 `WH_KEYBOARD_LL` hook в `services/kepler-backend/src/dictation/hotkey_hook.rs`. Parse accelerator → `Matcher`, emit broadcast `dictation_ptt_trigger { phase }`. Electron skip'ает globalShortcut, listens hook. Windows-only.
- **HTTP/SOCKS proxy** — поле `httpProxy` в `dictation-config.json`, `reqwest::Proxy::all(url)`. Feature `socks` в reqwest. Ортогонально DoH.
- **Custom Whisper prompt** — поле `transcriptionPrompt`, multipart `prompt` field в Groq запросе.
- **Playwright e2e** — `shell/e2e/dictation.spec.ts` (3 теста headless contract).

## Out of scope (Phase 2+)

- Локальные модели (whisper.cpp через `whisper-rs`, Parakeet V3,
  download manager UI, CUDA acceleration).
- LLM post-processing транскрипта (filler-word cleanup, пунктуация,
  стиль под контекст приложения).
- DoH для не-AI запросов (sync/RAWG/прочее).
- macOS / Linux порт (enigo cross-platform, hickory тоже, но
  Win32 GetForegroundWindow + Credential Manager + low-level hook —
  Windows-only).
- Persistent история транскрипций.
- Лимит 60+ sec за один запрос (Groq batch limit).

## Acceptance criteria

1. ✅ `cargo build --manifest-path
services/kepler-backend/Cargo.toml --bin kepler-backend` зелёный.
2. ✅ `cargo test --lib` зелёный, включая новые unit-тесты:
   - `dictation::config` round-trip (JSON load/save).
   - `dictation::network::build_client` для всех 4 NetworkProfile
     не паникует (компилируется + строит client). Реальный resolve
     gated за `KOSMOS_NETWORK_TESTS=1`.
   - `dictation::host` state machine transitions (Idle→Recording→
     Transcribing→Idle, cancel из любого state).
   - `dictation::inject` smoke (mock enigo через trait? для MVP —
     просто компиляция; реальная работа в manual verify).
3. ✅ TypeScript build `bun run --cwd shell typecheck` зелёный.
4. ✅ После `bun run dev`:
   - Settings → "Безопасность" видна, переключение DNS-профиля
     сохраняется в `dictation-config.json`.
   - Settings → "Диктация" (advanced) видна, установка API key
     приводит к `has_api_key: true` в `dictation.get_state`.
5. ✅ Manual verify (visual): hotkey `Ctrl+Shift+;` → pill
   появляется → говорю "тест диктации раз два три" → отпускаю
   (toggle второе нажатие) → текст вставлен в активный редактор
   (VS Code или Telegram).
6. ✅ Negative case: невалидный API key → понятная ошибка в pill,
   не белый экран.
7. ✅ Negative case: нет интернета → понятная ошибка, не stuck в
   Transcribing.
8. ✅ Clipboard contract: после inject оригинальное содержимое
   clipboard восстановлено (проверить вручную — скопировать что-то
   до hotkey, выполнить диктовку, Ctrl+V в другом месте → старый
   контент).
9. ✅ Focus contract: pill **не** забирает focus с активного
   окна (visual — Telegram остаётся подсвечен в taskbar пока pill
   виден).
10. ✅ Headless contract: `KOSMOS_HEADLESS=1` — hotkey НЕ
    регистрируется, pill window НЕ показывается (Playwright e2e).
11. ✅ `bun run ark:guard:writes` зелёный (dictation не пишет
    в ARK таблицы).
12. ✅ `bun run ark:smoke` зелёный.
13. ✅ `bun run docs:check` зелёный.
14. ✅ `bunx playwright test --config shell/playwright.config.ts`
    зелёный (включая новый `dictation.spec.ts`).
15. ✅ Регрессия: launcher Alt+Space + focus mode hotkey
    продолжают работать, новый hotkey не конфликтует.

## Architecture decisions

### Почему backend = Rust, audio capture = renderer?

- Renderer уже имеет Web Audio API + getUserMedia из коробки. Дублировать
  через `cpal` в Rust — лишний native dep на Phase 1.
- Транскрипция через cloud — bytes как угодно доезжают до Groq, нет
  выигрыша от capture в Rust.
- Phase 2 (локальные модели) — пересмотрим: для real-time стриминга в
  whisper.cpp вероятно cpal в Rust будет выгоднее.

### Почему keyring для API key?

- Enterprise-стандарт (Raycast → macOS Keychain, gh → Credential Manager,
  git-credential-manager). Encryption per-user на ОС-уровне.
- Не утекает через `%APPDATA%\Kosmos\` бэкап (`db_backup` копирует ark.db
  и могут случайно начать копировать и config.json).
- Не утекает через случайный коммит, crash dump, screen sharing
  `dictation-config.json` (всё что не секрет — да, но без ключа).

### Почему отдельная `Безопасность` страница, а не часть `Диктация`?

- DNS resolver — generic feature для всех AI-провайдеров (будущих).
  Логически принадлежит `Сеть/Безопасность`, не одной фиче.
- Открывает место для будущих security опций (proxy, network kill-switch,
  telemetry toggle, biometric для key access).

### Почему toggle в Phase 1, не push-to-talk?

- `globalShortcut.register` в Electron даёт только key-down event,
  не key-up. Push-to-talk требует low-level hook (Rust-side
  `SetWindowsHookExW` или JS `node-global-key-listener`).
- Это отдельный архитектурный слой (low-level key hook running в
  Rust process, бридж к hotkey config). Worth a separate proof-loop.
- Raycast/Handy используют OS APIs для key-up; в Phase 2 добавим.

### Почему batch submit, не streaming chunks?

- Groq endpoint не принимает streaming audio — это transcription
  endpoint, не realtime ASR.
- Streaming усложняет state machine (Recording с буфером VS
  Transcribing с pending chunks) без выигрыша.
- Phase 2 локальные модели поддерживают streaming через AudioWorklet,
  тогда и добавим.

### Почему `windows` crate, не Electron-side для focus restore?

- У нас уже native слой (focus mode helper использует `windows` crate).
- Electron-side нужен FFI / отдельный addon — лишний build complexity.
- `GetForegroundWindow` / `SetForegroundWindow` — простые синхронные
  Win32 calls, идеально для Rust.

### Why DoH, not HTTP proxy in Phase 1?

- DNS poisoning — самый частый блок Groq в РФ. DoH решает 80% случаев.
- HTTP proxy усложняет config (host, port, auth, scheme), требует
  понимания пользователем сетевого стека.
- DoH работает прозрачно, юзер просто выбирает "Cloudflare" из списка.
- Roadmap: proxy support когда выяснится что SNI-block превалирует.

## Proof loop

При завершении заполнить `evidence.md` со снимками:

- `cargo test --lib` output (passing).
- `bun run --cwd shell typecheck` output.
- Screenshot Settings "Безопасность" с DNS picker.
- Screenshot Settings "Диктация" с API key полем.
- Screenshot pill в реальном использовании.
- Текст до/после Ctrl+V (доказательство clipboard restore).
- `bun run ark:guard:writes`, `ark:smoke`, `docs:check` output.

## Estimate (от skill estimate-calibration)

- **Human gut:** ~60h (~7-10 рабочих дней с ревью + integration).
- **LLM wall-clock:** ~40 мин (anchored на app-launcher 0.32h actual,
  +overhead на новые native deps).
