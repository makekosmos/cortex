# Evidence

## AC1 — PASS

Проверка:

- `cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation::host::tests::mock_transcript_override_requires_test_mode -- --exact --nocapture`

Результат:

- helper `mock_dictation_transcript_override("mock")` возвращает `None` без `KOSMOS_TEST_MODE` / `KOSMOS_HEADLESS`
- при `KOSMOS_TEST_MODE=1` и `KOSMOS_TEST_DICTATION_TRANSCRIPT=...` возвращает детерминированный transcript
- для `provider="groq"` bypass не включается даже в test mode

Вердикт: `PASS`

## AC2 — PASS

Проверка:

- `cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation::host::tests::submit_audio_mock_transcript_succeeds_without_api_key_and_cleans_up -- --exact --nocapture`

Результат:

- `start_recording` -> `submit_audio` проходит успешно без API key
- test ловит `dictation_state_changed(state=transcribing)`
- затем приходит `dictation_transcript` с детерминированным текстом
- `get_state` возвращает `idle`
- `list_pending` пустой
- `get_stats` показывает `totalSessions=1`, `totalWords=3`, `totalRecordSeconds=4`

Вердикт: `PASS`

## AC2b — PASS

Проверка:

- `cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation::host::tests::submit_audio_groq_transcript_succeeds_with_test_api_key_and_cleans_up -- --exact --nocapture`

Результат:

- `provider="groq"` path проходит через `start_recording` -> `submit_audio` -> `process_one_attempt`
- HTTP транскрипция мокается через `httpmock`, реальный Groq/network не используются
- runtime читает `KOSMOS_TEST_GROQ_API_KEY` под `ENV_DICTATION_TEST_LOCK`
- транскрипт, stats, pending cleanup и возврат в `idle` проходят на production-like пути
- `injectMode` фиксирован как `clipboard_only`

Вердикт: `PASS`

## AC3 — PASS

Проверка:

- `cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation::host::tests::resolve_attempt_inject_mode_forces_clipboard_only_for_mock -- --exact --nocapture`
- code inspection в `platform/runtime/src/dictation/host.rs`

Результат:

- mock path переводит `auto_paste` в `clipboard_only` на уровне runtime helper'а
- в mock branch OS inject вообще пропускается, а transcript event помечается как `injected=false`
- renderer/UI platform-specific веток не добавлялось

Вердикт: `PASS`

## AC4 — PASS

Проверка:

- `rtk cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation`

Результат:

- `145 passed, 3 ignored, 203 filtered out (1 suite, 2.09s)`
- новый coverage живёт рядом с runtime host tests и не требует Groq / микрофона

Вердикт: `PASS`

## AC5 — PASS

Проверка:

- `rtk cargo fmt --manifest-path platform/runtime/Cargo.toml -- platform/runtime/src/dictation/host.rs`
- `rtk cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation`

Результат:

- оба запуска потребовали эскалацию из-за локального доступа к `.agent/tasks` и `target\debug\.cargo-lock`; после эскалации команды выполнились
- e2e не добавлялся: для этого slice достаточно focused runtime proof

Вердикт: `PASS`

## Follow-up smoke coverage — wired, now passed

Проверка:

- code inspection в `platform/desktop/e2e/dictation.spec.ts`
- opt-in Playwright smoke contract:
  `rtk bun run --cwd platform/desktop playwright test --config playwright.config.ts e2e/dictation.spec.ts --grep "opt-in real provider path works headless without microphone"`

Результат:

- Добавлен headless-only smoke для реального `provider="groq"` без микрофона: тест берёт WAV из `KOSMOS_TEST_DICTATION_AUDIO_B64`, а если он не задан, synthesizes tiny WAV fallback; ключ по-прежнему берётся из `KOSMOS_TEST_GROQ_API_KEY`, `injectMode="clipboard_only"` и путь проходит через `start_recording -> submit_audio`.
- Opt-in real Groq smoke был available и в этой сессии уже passed при установленном `KOSMOS_TEST_GROQ_API_KEY`.
- Transcript assertion skipped, because no expected substring was set.
- Для headless/test mode runtime читает `KOSMOS_TEST_GROQ_API_KEY` напрямую, поэтому smoke не трогает реальный keyring пользователя.
- Success path ожидает `idle`, пустой pending queue, ненулевые stats и непустой clipboard transcript; опционально проверяется `KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING`.

Вердикт: `WIRED (local run requires API key; audio fixture optional)`

## Completion audit

Automated proofs are complete:

- AC1 through AC5 are PASS.
- `rtk bun run --cwd platform/desktop playwright test --config playwright.config.ts e2e/dictation.spec.ts` completed with `7 passed, 1 skipped` on the base suite, then the opt-in real Groq smoke was run separately in this session and passed.
- The separate opt-in real Groq smoke run completed with `1 passed (13.1s)`.
- Fresh backend rebuild evidence: `rtk cargo build --manifest-path platform/runtime/Cargo.toml --bin kepler-backend` finished the dev profile in `14m 37s`.

Opt-in real Groq smoke is available and passed in this session:

- wrapper: `node scripts/run-dictation-groq-smoke.mjs`
- direct command: `node <resolved-playwright-cli> test --config platform/desktop/playwright.config.ts --reporter=line --output <temp> platform/desktop/e2e/dictation.spec.ts -g "opt-in real provider path works headless without microphone"`
- required env var: `KOSMOS_TEST_GROQ_API_KEY`
- optional env vars: `KOSMOS_TEST_DICTATION_AUDIO_B64`, `KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING`
- when audio is omitted, the wrapper and smoke synthesize a tiny WAV fixture locally
- observed runtime logs: backend ready, config updated for groq, recording started, submit_audio done with state idle, clipboard updated
- transcript assertion stayed skipped because no expected substring was set

Manual residuals stay pending until a human runs them on real devices:

- Реальный hotkey smoke на Windows/macOS, разрешения микрофона, точность транскрипта и `auto_paste` behaviour остаются manual-only и не считаются закрытыми, пока пользователь не прогонит их сам.
- Короткий чеклист: `.agent/tasks/2026-06-19-dictation-mock-stt/manual-smoke-checklist.md`
- Канонический pending checklist теперь продублирован в `docs-site/agents/manual-tests-pending.md`.

## Verification refresh (this turn)

Проверка:

- `.\\node_modules\\.bin\\tsc.exe --noEmit` из `platform/desktop`
- `cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation::config`
- `.\\node_modules\\.bin\\playwright.exe test --config playwright.config.ts e2e/dictation.spec.ts` из `platform/desktop`
- `rtk cargo build --manifest-path platform/runtime/Cargo.toml --bin kepler-backend`

Результат:

- TypeScript check сначала упёрся в sandbox `EPERM` на `platform/desktop/.tsbuildinfo`; повторный запуск вне sandbox прошёл.
- Focused runtime config tests: `5 passed, 346 filtered out`.
- Focused Playwright run сначала упёрся в sandbox `EPERM` на `platform/desktop/test-results/.last-run.json`; повторный запуск вне sandbox дал `7 passed, 1 skipped`, где skipped — новый opt-in Groq smoke без секретов/аудио.
- Backend rebuild finished the dev profile in `14m 37s`.
