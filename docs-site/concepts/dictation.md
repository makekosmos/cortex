# Dictation (STT)

Источник: `services/kepler-backend/src/dictation/` + `shell/electron/dictation-pill.ts` + `shell/src/views/DictationPillView.vue`.

## За 30 секунд

Dictation — голосовой ввод по образцу [Raycast Dictation](https://manual.raycast.com/ai/dictation). Global hotkey → плавающая **pill** (320×64, frameless, alwaysOnTop, `focusable: false`) → запись микрофона через Web Audio API → транскрипция в **Groq Cloud** (whisper-large-v3) → **auto-paste** транскрипта в активное окно через симуляцию Ctrl+V.

Phase 1 — облачная транскрипция. Phase 2 (roadmap) — локальные модели через whisper.cpp / Parakeet V3.

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
POST api.groq.com/openai/v1/audio/transcriptions  (reqwest + опциональный DoH resolver)
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

| Файл                                                               | Что делает                                                                                                                                                                                                                    |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `services/kepler-backend/src/dictation/host.rs`                    | State machine `Idle → Recording → Transcribing → Idle\|Error`. Dispatch для `dictation.*` operations. `broadcast::Sender<Value>` для WS events.                                                                               |
| `services/kepler-backend/src/dictation/groq.rs`                    | Multipart POST к `/openai/v1/audio/transcriptions`. `language="auto"` → пропускаем поле (Whisper auto-detect).                                                                                                                |
| `services/kepler-backend/src/dictation/inject.rs`                  | `enigo` Ctrl+V симуляция + `arboard` clipboard save/restore + `windows::Win32 GetForegroundWindow/SetForegroundWindow` для возврата фокуса. Все sleep'ы 80ms.                                                                 |
| `services/kepler-backend/src/dictation/config.rs`                  | JSON (`<data_dir>/dictation-config.json`) для не-секретов + `keyring` (Windows Credential Manager target `kosmos-kepler` / user `groq-api-key`) для API ключа.                                                                |
| `services/kepler-backend/src/dictation/network.rs`                 | Фабрика `reqwest::Client` с переключаемым DNS resolver'ом (System / Cloudflare DoH / Google DoH / custom DoH URL) через `hickory-resolver`. Scope: только AI HTTP.                                                            |
| `shell/electron/dictation-pill.ts`                                 | BrowserWindow lifecycle (`focusable: false`, alwaysOnTop, transparent), global hotkey регистрация, IPC `kepler:dictation:toggle/cancel/pill-finished/command`. Hotkey перерегистрируется на event `dictation_config_changed`. |
| `shell/src/views/DictationPillView.vue`                            | Audio capture (Web Audio API), waveform (AnalyserNode → 12 bars), таймер, WAV encode, base64, отправка через `window.kepler.ark.request("dictation.submit_audio", ...)`.                                                      |
| `shell/src/views/SettingsView.vue` (tabs "security" + "dictation") | UI настроек: DNS resolver выбор (Безопасность, main group), hotkey / язык / inject mode / API key / тест (Диктация, advanced group).                                                                                          |

## Безопасность ключа

API key Groq хранится в **Windows Credential Manager** через крейт `keyring` (target `kosmos-kepler`, user `groq-api-key`). Это enterprise-стандарт (так делают Raycast на macOS Keychain, gh CLI, git-credential-manager).

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

`globalShortcut.register` шлёт только key-down event'ы — для hold-to-record этого мало. Решение: Win32 low-level keyboard hook (`WH_KEYBOARD_LL`) в выделенном OS-потоке (`services/kepler-backend/src/dictation/hotkey_hook.rs`). Hook парсит accelerator из config'а в `Matcher { vk, ctrl, shift, alt }`, на каждое key event'е проверяет vk + модификаторы через `GetAsyncKeyState`, emit'ит broadcast `dictation_ptt_trigger { phase: "down" | "up" }`.

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

## Выбор модели

UI не даёт менять модель — вшит **`whisper-large-v3`**. Почему именно он:

- **whisper-large-v3** — самая качественная среди Groq Whisper'ов: лучше держит длинный контекст и пунктуацию, корректнее работает с не-английской речью. Дороже на токен, но не критично.
- **whisper-large-v3-turbo** — ~2.5× быстрее, но на русской речи замечены пропуски слов и более частые галлюцинации на низкоуровневом шуме. Не подходит для default'а.
- **distil-whisper-large-v3-en** — только английский.

Если ручка нужна — можно вручную поправить `model` в `dictation-config.json` (configurable hook сохраняется). При появлении локальных моделей в Phase 2 модель снова окажется в UI.

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

## Что НЕ делает Phase 1 / 1.5

- Локальные модели (whisper.cpp, Parakeet V3) — отдельный proof-loop Phase 2.
- LLM post-processing транскрипта (filler-word cleanup, пунктуация, стиль) — Phase 3.
- DoH для не-AI запросов (sync / RAWG / прочее) — out of scope.
- macOS / Linux порт — Win32 `GetForegroundWindow` + Credential Manager + `WH_KEYBOARD_LL` специфичны.
- Persistent история транскрипций.
- PTT на macOS / Linux (low-level hook Windows-only в Phase 1.5).

## ADR

См. `reference/decisions.md` § 2026-05-24 — «Phase 1 STT — облачный Groq, не локальный whisper.cpp».
