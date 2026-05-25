# 2026-05-25 dictation resilience (Phase 2 — network/retry/DoH)

## Context

Phase 1 + 1.5 диктации (Groq cloud, push-to-talk, proxy, prompt) закрыты в
0.3.0–0.3.2. По факту использования вскрылись три класса проблем:

1. **Audio loss at network failure.** `host.rs::do_transcribe_and_inject`
   синхронно: base64 → POST Groq → inject. Любая сетевая ошибка (DNS,
   connect, 5xx, 401, timeout) → state `Error` → pill закрывается через
   1.2s → PCM buffer обнулён в renderer. Юзер наговорил 30 секунд → нет
   сети → всё потеряно, пересказывай. Нет autosave, нет retry, нет очереди.
2. **Custom DoH не работает.** `network.rs:54-62` для
   `NetworkProfile::CustomDoh` молча фолбэчится на Cloudflare с
   `eprintln!` warning. TODO Phase 1.5 не сделан. UI ручка «свой DoH URL»
   обманывает: что бы юзер ни ввёл — идёт Cloudflare. Валидации URL в
   `op_update_config` (host.rs:332-338) нет — пустая строка/без `https://`
   спокойно сохраняется.
3. **Диагностика недоступна.** `tracing` подключён в Cargo.toml, в
   dictation модуле не используется. `op_test_connectivity`
   (host.rs:608-630) пакует ошибку только в JSON-ответ, в backend log
   ничего не пишет. UI показывает плоское «Не удалось» —
   DNS-fail/proxy-fail/401/timeout не различимы. Юзер не понимает что
   крутить, разработчик не понимает что чинить.

Прецеденты для retry/persist UX (см. research-блок в conversation):

- **Wispr Flow** — WAV пишется на диск ДО HTTP, history pending 14 дней,
  retry через клик на pill/history. Toast «No internet — audio saved».
- **MacWhisper** — Retry-кнопка на Error state.
- **AWS Builders' Library + BoldSign** — exponential backoff с jitter
  (base 200ms, cap 30s, 3-5 попыток), idempotency UUID, retry **только** на
  `is_timeout`/`is_connect`/429/5xx, **не** на 4xx (кроме 429).

## Scope

### Backend — `services/kepler-backend/src/dictation/`

#### 1. `pending.rs` (новый) — persistent очередь

- `PendingItem { uuid: Uuid, wav_path: PathBuf, created_at: DateTime<Utc>,
attempts: u32, last_error: Option<String>, language: Option<String>,
prompt: Option<String> }`.
- Хранится в `<dataDir>/dictation/pending/`:
  - `<uuid>.wav` — сырое аудио (16kHz/16-bit/mono, как сейчас приходит из
    renderer).
  - `<uuid>.json` — метаданные (`PendingItem` без `wav_path`).
- API:
  - `pub fn enqueue(host: &DictationHost, wav: &[u8], opts: TranscribeOpts)
-> Result<Uuid>` — пишет файлы атомарно (`*.tmp` → rename).
  - `pub fn list(data_dir: &Path) -> Result<Vec<PendingItem>>` — сканирует
    директорию, сортирует по `created_at` ASC.
  - `pub fn drop(data_dir: &Path, uuid: Uuid) -> Result<()>` — удаляет оба
    файла. Вызывается после успешного inject.
  - `pub fn bump_attempt(data_dir: &Path, uuid: Uuid, err: &str) ->
Result<()>` — `attempts += 1`, `last_error = Some(err)`, перезаписать
    JSON.
- **GC**: на app start удалять элементы старше 7 дней ИЛИ если их
  больше 20 (LRU). См. forbidden ниже — НЕ 14 дней Wispr-style.

#### 2. `retry.rs` (новый) — backoff + классификация ошибок

- `pub enum FailureKind { Retryable, Fatal { user_msg: &'static str } }`
- `pub fn classify(err: &SubmitError) -> FailureKind`:
  - `SubmitError::Network(reqwest::Error)` →
    - `is_timeout() || is_connect() || is_request()` → `Retryable`
    - всё прочее (TLS handshake fail, body error) → `Retryable`
      (consider conservative — лучше retry чем потеря)
  - `SubmitError::Groq(GroqError::Api { status, .. })`:
    - `status == 429` → `Retryable`
    - `status >= 500 && < 600` → `Retryable`
    - `status == 401` → `Fatal { "Неверный API key" }`
    - `status == 413` → `Fatal { "Аудио слишком длинное" }`
    - `status == 400` → `Fatal { "Groq отклонил запрос" }`
    - прочее 4xx → `Fatal { "Ошибка Groq {status}" }`
  - `SubmitError::Groq(GroqError::Http(e))` → классифицировать как Network.
  - `SubmitError::NoApiKey` → `Fatal { "API key не задан" }`
  - `SubmitError::AudioDecode(_)` → `Fatal { "Битое аудио" }`
  - `SubmitError::Inject(_)` → `Fatal { "Не удалось вставить текст" }`
    (но текст уже расшифрован — см. ниже про clipboard fallback)
- `pub async fn with_backoff<F, Fut, T>(attempts: u32, op: F) -> Result<T,
SubmitError>` где между попытками `tokio::time::sleep(base * 2^n +
jitter)`. Base = 500ms, cap = 8s. По умолчанию `attempts = 3`. После
  первой `Fatal` — break без retry.

#### 3. `host.rs` — переработка `do_transcribe_and_inject`

Новый поток:

```
op_submit_audio(audio_b64, opts):
  1. decode base64 → wav_bytes
  2. uuid = pending::enqueue(host, &wav_bytes, opts)  // disk write FIRST
  3. transition → Transcribing { uuid }
  4. spawn tokio task: process_pending(host, uuid)
  5. вернуть { ok: true, uuid }   // ack сразу, не блокируем WS

process_pending(host, uuid):
  load pending item
  result = retry::with_backoff(3, || groq::transcribe(client, wav))
  match result:
    Ok(text):
      pending::drop(uuid)
      inject_result = inject::inject(text, mode, prev_hwnd)
      match inject_result:
        Ok: transition → Idle, emit transcript
        Err: emit transcript anyway (text в clipboard),
             transition → Error { kind: InjectFailed, transcript: text }
    Err(e):
      kind = classify(&e)
      pending::bump_attempt(uuid, &e.to_string())
      match kind:
        Retryable: transition → Pending { uuid }, NOT Error
        Fatal { msg }: transition → Error { user_msg, can_retry: true, uuid }
```

- Добавить state variants:
  - `DictationState::Pending { uuid, attempts }` — visible на pill как
    серый dot, не auto-close. Юзер видит «ждёт сети».
  - `DictationState::Error { user_msg, can_retry, uuid? }` —
    pill жёлтый/оранжевый, **не auto-close через 1.2s** (см. forbidden).
- Новые WS endpoints:
  - `dictation.list_pending` → `[{ uuid, created_at, attempts, last_error,
duration_sec }]` для UI.
  - `dictation.retry { uuid }` → `process_pending(host, uuid)` если не
    запущен.
  - `dictation.discard { uuid }` → `pending::drop`.
  - `dictation.retry_all` → запустить retry для всех pending.
- On app start (в `DictationHost::new` / `setup`): `pending::list` →
  если есть, broadcast `dictation_pending_changed`, фронт показывает
  badge в Settings → Диктация и в pill через notification (toast в shell).

#### 4. `network.rs` — реализовать CustomDoH + cache

- Парсить `NetworkProfile::CustomDoh { url }` через `url::Url`:
  - schema == "https", non-empty host, path содержит `/dns-query` (если
    нет — добавить).
  - Резолвить IP резолвера через системный DNS однократно при build
    (пока bootstrap), складывать в `NameServerConfig { socket_addr,
protocol: Protocol::Https, tls_dns_name: Some(host), ... }`.
  - **Если не удалось** парсить — `build_client` возвращает `Err`
    с понятным сообщением (НЕ silent fallback).
- **Кэширование**: `DictationHost` хранит `Arc<RwLock<Option<(Client,
NetworkProfile, Option<String>)>>>`. Инвалидация на
  `dictation_config_changed` если профиль или proxy менялись. Сейчас
  клиент пересоздаётся на каждый submit/test — лишний overhead +
  скрывает что resolver висит на старте.
- **Tracing**: `tracing::info!` / `warn!` / `error!` в `build_client`,
  `build_resolver`, custom URL parse, proxy parse. На каждом error path.

#### 5. `host.rs::op_update_config` — валидация

- Если `network_profile = CustomDoh` и URL пустой/без `https://` → return
  Err с сообщением «Custom DoH URL должен начинаться с https://».
- Если `http_proxy` непустой и не парсится `reqwest::Proxy::all(&url)` →
  Err с сообщением.

#### 6. `host.rs::op_test_connectivity` — детальная диагностика

Возврат:

```jsonc
{
  "ok": true,
  "stages": {
    "client_build": { "ok": true, "ms": 12 },
    "dns_resolve": { "ok": true, "ms": 84, "ip": "104.18.32.115" },
    "tcp_connect": { "ok": true, "ms": 145 },
    "tls_handshake": { "ok": true, "ms": 230 },
    "http_head": { "ok": true, "status": 200, "ms": 410 },
  },
  "total_ms": 881,
}
```

При fail — соответствующая стадия с `ok: false, error: "..."`. Frontend
показывает первый failing stage с понятным текстом («DNS не разрешается
— попробуйте Cloudflare DoH», «TCP connect refused — проверьте proxy»,
«TLS handshake — возможно SNI блок», «HTTP 401 — неверный API key»).

#### 7. Tracing в dictation модуле

- В Cargo.toml уже `tracing` + `tracing-subscriber`. Добавить
  `use tracing::{debug, error, info, warn};` в `host.rs`, `groq.rs`,
  `network.rs`, `pending.rs`, `retry.rs`.
- Минимум:
  - `info!` на каждой state transition (`Idle → Recording → ...`).
  - `warn!` на retryable error с попыткой N.
  - `error!` на fatal error с classification.
  - `debug!` на стадии test_connectivity.

### Cargo deps

- `uuid = { version = "1", features = ["v4", "serde"] }` — если ещё нет
  глобально, проверить workspace Cargo.toml.
- `url = "2"` — для custom DoH парсинга (вероятно уже в графе через
  reqwest).
- `chrono` — уже есть.

### Renderer — `shell/src/`

#### `DictationPillView.vue`

- Новый state `pending` — серый кружок + текст «Ждёт сети, попытка N».
  НЕ auto-close.
- `error` state с `can_retry = true` — добавить кликабельную retry-иконку
  справа от `!`. Клик → `window.kepler.dictation.retry(uuid)`. НЕ
  auto-close через 1.2s, остаётся пока юзер не нажал retry/cancel или не
  истёк 30s idle timeout (после идёт hide, item остаётся в pending).
- Тексты ошибок — из backend `user_msg`, не raw error string.

#### `SettingsView.vue` / `DictationPage.vue`

- Новая секция «Очередь диктовок» — список pending items:
  - `dd.mm HH:MM · {duration}s · попыток {N} · {last_error}`
  - Кнопки: «Повторить», «Удалить».
  - Кнопка «Повторить все».
  - Скрыть секцию если очередь пустая.
- Test connectivity — рендерить detailed stages (см. формат выше) — список
  с галочкой / крестиком по стадиям и временем мс. Первая failing
  подсвечена красным с подсказкой.
- Custom DoH URL input — `<input @blur>` валидация на клиенте:
  https-schema + non-empty host. Кнопка save disabled пока невалидно.
  Hint под полем: «Пример: `https://comss.dns.controld.com/dns-query`».

### Shell (Electron)

- `preload.ts` — добавить `listPending`, `retry(uuid)`, `discard(uuid)`,
  `retryAll`, `onPendingChanged(cb)` в `window.kepler.dictation`.
- `main.ts` — wire IPC к WS endpoints.
- На app start (после backend up) — `dictation.list_pending`. Если
  непусто → system notification «N диктовок ждут отправки. Открыть очередь?»
  (одна нотификация, не на каждый item). НЕ блокирует UI.

### Doc updates

- `docs-site/concepts/dictation.md` — секция «Resilience»: pending queue,
  retry classification, что delivers / что нет.
- `docs-site/agents/forbidden.md` — секция Dictation, добавить:
  - ❌ Удалять `<uuid>.wav` из pending до успешного inject (или явного
    discard юзером). Audio loss = unrecoverable.
  - ❌ Retry на 4xx (кроме 429). 401/400 — caller bug, retry — пустая
    трата квоты.
  - ❌ Silent fallback на Cloudflare для custom DoH. Если URL невалидный
    — error, не «как будто работает».
  - ❌ Pending queue > 20 items / older than 7 days. Disk leak,
    privacy issue (длинные диктовки лежат в plaintext WAV).
  - ❌ Pending WAV вне `<dataDir>/dictation/pending/` (instance isolation
    нарушится в тестах).
  - ❌ Pill auto-close на `Error { can_retry: true }` через 1.2s.
    Юзеру нужно успеть кликнуть retry. Min 30s или клик cancel.
- `docs-site/reference/decisions.md` — ADR:
  «Phase 2-resilience: persist-before-upload + retry queue вместо
  inline-retry-only. Причина: при network drop renderer уже обнулил
  PCM buffer, у backend нет аудио для retry. Disk-first гарантирует
  recovery даже при crash backend / kill app.»
- `bun run docs:sync` → `bun run docs:check`.

## Out of scope (Phase 3+)

- Локальный fallback на whisper.cpp при недоступности Groq (отдельный
  крупный proof-loop, упоминался в Phase 2 «Out of scope» исходного spec).
- LLM post-processing транскриптов.
- macOS/Linux port.
- Persistent история **успешных** транскриптов (история ≠ retry-queue).
  Pending — temporary holding, не архив.
- Streaming partial results.
- Группировка retry в bulk-request (Groq не поддерживает batch).

## Acceptance criteria

1. ✅ `cargo build --manifest-path services/kepler-backend/Cargo.toml --bin
kepler-backend` зелёный.
2. ✅ `cargo test --lib` зелёный, включая новые unit-тесты:
   - `dictation::pending` — enqueue → list → drop round-trip,
     atomic write (tmp + rename), GC по возрасту и количеству.
   - `dictation::retry::classify` — каждый вариант `SubmitError` →
     ожидаемый `FailureKind`.
   - `dictation::retry::with_backoff` — first-attempt success, retry на
     mock retryable error, no-retry на mock fatal error.
   - `dictation::network` — `CustomDoh { url: "not a url" }` → `Err`,
     `CustomDoh { url: "https://comss.dns.controld.com/dns-query" }` →
     `Ok` (build only, реальный resolve gated за `KOSMOS_NETWORK_TESTS=1`).
3. ✅ `bun run --cwd shell typecheck` зелёный.
4. ✅ Manual verify (visual): отключаю Wi-Fi → диктую → отпускаю →
   pill серый «ждёт сети» → возвращаю Wi-Fi → диктовка автоматически
   доходит до inject в течение ≤10s.
5. ✅ Manual verify: kill `kepler-backend` mid-transcribing → перезапуск →
   Settings → Диктация → очередь содержит item → «Повторить» → текст
   приходит в clipboard (или inject если pill хотя бы один раз был открыт).
6. ✅ Custom DoH: ввожу `https://comss.dns.controld.com/dns-query` →
   Save → Test connectivity → stages показывают `dns_resolve.ok = true`
   с реальным IP.
7. ✅ Custom DoH: ввожу `not a url` → save disabled на frontend, backend
   reject если обойти frontend.
8. ✅ Test connectivity при выключенной сети → первый failing stage
   подсвечен, текст подсказки понятный.
9. ✅ 401 (неверный API key) → НЕ retry, сразу Error с понятным текстом
   «Неверный API key». Pending item НЕ создаётся (или сразу удаляется —
   решить в impl: проще не создавать).
10. ✅ Pending GC: создать 25 фейковых pending → restart → остаётся 20
    самых свежих. Создать pending старше 7 дней (mtime back-dated) →
    restart → удалён.
11. ✅ `bun run ark:guard:writes` зелёный (pending файлы не в ARK).
12. ✅ `bun run ark:smoke` зелёный.
13. ✅ `bun run docs:check` зелёный.
14. ✅ Headless contract сохранён: `KOSMOS_HEADLESS=1` — pill не
    показывается, system notification про pending не вылетает.
15. ✅ Tracing видна: `RUST_LOG=kepler_backend::dictation=debug` →
    stages connectivity, retry attempts, classify decisions — всё в
    log.
16. ✅ Регрессия: остальная диктация работает как раньше — happy path
    Recording → Transcribing → Idle без задержек, push-to-talk и toggle
    оба работают.

## Architecture decisions

### Почему persist-to-disk ДО HTTP, а не in-memory queue?

- Backend crash, OS reboot, app kill — всё уносит in-memory. WAV на
  диске переживает.
- Размер: 30s × 16kHz × 16bit mono = 960 KB. 20 items = ~19 MB. Дёшево.
- Renderer всё равно базируется на disk (нет другого надёжного места
  для long-running buffer).
- Cleanup — простая GC задача при старте.

### Почему не отдельный воркер-таск, а spawn в `process_pending`?

- Воркер-цикл (1 потребитель, очередь Mutex'ом) — следующий шаг если
  понадобится. Сейчас одна диктовка в момент времени — race нет.
- Если юзер быстро запускает 3 раза подряд при отсутствии сети — это
  3 параллельных task'а на 3 разных uuid. Не страшно: Groq просто
  отклонит rate limit'ом или всё пройдёт. UUID гарантирует
  идемпотентность.
- Phase 3: добавить mutex / channel если станет проблемой.

### Почему retry с jitter, а не fixed delay?

- AWS Builders' Library: thundering herd при сетевом восстановлении (все
  накопленные клиенты ломятся одновременно). Jitter ±50% от base.
- 3 попытки 500ms / 1s / 2s + jitter — суммарно <10s в худшем сценарии.
  Юзер не успеет начать новую диктовку.

### Почему не fallback на локальную модель сейчас?

- Это Phase 3 (большой proof-loop). Resilience-фикс не зависит от него:
  persist + retry полезны даже когда Phase 3 появится (тогда retry будет
  cloud первая, потом local-fallback по флагу).

### Почему детальные stages в test_connectivity, а не один OK/Fail?

- РФ-блокировка может бить на любой стадии: DNS poisoning (Cloudflare
  DoH лечит), SNI block (нужен proxy), IP block (нужен proxy с другим
  exit), 401 (юзер ввёл не тот ключ). Юзер сейчас видит «Не удалось» и
  не понимает что крутить. Стадии — diagnostic для самостоятельной
  настройки.

### Почему не сохранять transcript в pending для post-mortem?

- Privacy: pending WAV уже sensitive. Plaintext transcript — ещё хуже
  (легко grep'ается). Если inject упал, текст один раз попадает в
  clipboard и сразу теряется. Это осознанная цена.
- Альтернатива (Phase 3+): зашифрованная история через ARK objects с
  user-controlled retention.

### Почему 7 дней / 20 items, а не Wispr Flow 14 дней?

- Личный лаунчер, не SaaS. Wispr держит 14d потому что у них кросс-
  девайс sync и история-как-feature. У нас pending = recovery, не
  history. 7d + 20 items покрывает 95% сценариев («ноут в самолёте на
  день, дома доехало»).

## Proof loop

При завершении заполнить `evidence.md`:

- `cargo test --lib dictation` output.
- `bun run --cwd shell typecheck`.
- Screenshot Settings → Диктация с непустой очередью (mock через
  airplane mode).
- Screenshot Settings → Безопасность с detailed connectivity stages.
- Screenshot pill в state Pending (серый dot + текст).
- Screenshot pill в state Error с retry-кнопкой.
- Видео/GIF: airplane mode on → диктую → off → авто-доставка.
- `tracing` output на одной полной retry-сессии (показать classify
  decisions).
- `bun run ark:guard:writes`, `ark:smoke`, `docs:check`.

## Estimate (от skill estimate-calibration)

- **Human gut:** ~24h (~3 рабочих дня с ревью). Backend pending+retry — день,
  custom DoH + stages — полдня, frontend pending UI + connectivity stages — день,
  doc/test — полдня.
- **LLM wall-clock:** ~25-35 мин (anchored на Phase 1 dictation actual ~40
  мин при сравнимом объёме — здесь меньше новых deps, но больше state-machine
  edge cases).
