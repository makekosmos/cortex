# Dictation mock STT path for headless verification

## Goal

Добавить детерминированный test/headless-only путь для `dictation.submit_audio`,
который не требует реального микрофона, сети или Groq API key, но проходит через
основную runtime state machine диктации настолько полно, насколько это практично.

## Scope

В scope:

- `platform/runtime/src/dictation/host.rs`
- при необходимости `platform/runtime/src/dictation/groq.rs`
- тесты, доказывающие env-gated mock transcript path
- production-like `provider="groq"` path через `httpmock` и `KOSMOS_TEST_GROQ_API_KEY`

Вне scope:

- command/settings UI
- production-поведение диктации
- реальные сетевые вызовы, микрофон, Groq key

## Acceptance Criteria

**AC1.** В test/headless mode при установленном `KOSMOS_TEST_DICTATION_TRANSCRIPT`
операция `dictation.submit_audio` может завершиться успешно без реального Groq/API key,
а вне test/headless этого bypass нет.

**AC2.** Mock path проходит через существующий host flow: `start_recording` ->
`submit_audio` -> `transcribing` -> transcript/stats/pending cleanup и финальный
state, без сетевого запроса.

**AC3.** Mock path не добавляет unsafe auto-inject в тестах: в безопасной
headless/test конфигурации используется уже существующее безопасное поведение
(`clipboard_only` либо текущий safe fallback), без platform-specific веток в renderer/UI.

**AC4.** Есть focused automated coverage, которая доказывает и mock transcript
bypass, и production-like Groq path через runtime host API/состояние/статы/
очередь/clipboard evidence без реального Groq или микрофона.

**AC5.** Фокусные проверки для этого slice выполнены настолько полно, насколько
позволяет sandbox: минимум targeted runtime dictation tests; если e2e/check не
удаётся запустить из-за sandbox/permission/environment, это зафиксировано в evidence.

## Verification state

- Automated proofs are complete and recorded in `evidence.md` / `evidence.json`.
- The opt-in real Groq smoke is runnable now, and the preferred wrapper only
  needs `KOSMOS_TEST_GROQ_API_KEY`; it synthesizes a tiny WAV fixture when
  `KOSMOS_TEST_DICTATION_AUDIO_B64` is absent.
- Real Windows/macOS hotkey, mic, `auto_paste`, `clipboard_only`, and adapter
  sanity checks remain manual-only until the user runs them on a live device.
