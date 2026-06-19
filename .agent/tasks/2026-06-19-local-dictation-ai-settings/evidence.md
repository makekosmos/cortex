# Evidence

## Implemented

- Added runtime `provider = "local"` dispatch for dictation.
- Added local dictation config fields:
  - `localEngine`
  - `localModelPath`
  - `localCommandPath`
  - `localModel` / `localModelId`
- Added `platform/runtime/src/dictation/local.rs` external `whisper.cpp` adapter:
  - writes submitted WAV bytes to a temporary file;
  - invokes user-configured `whisper-cli.exe` / compatible executable with `-m`, `-f`,
    `-otxt`, `-of`, optional `-l`, optional `--prompt`;
  - reads transcript from stdout or generated `.txt`;
  - cleans temporary files.
- Existing Groq and mock provider paths remain routed through the same state machine.
- Added advanced Settings `AI` page using existing `@kosmos/visuals` settings rows.
- Added dictation config composable support for local model/command fields.
- Added a runtime-owned local model catalog and auto-setup flow:
  - `dictation.list_local_models`
  - `dictation.download_local_model`
  - `dictation.use_local_model`
- Added Kosmos app-data storage for downloaded dictation models under
  `models/dictation` and Windows `whisper.cpp` tools under `tools/dictation/whisper.cpp`.
- Added Handy-inspired Whisper catalog entries for Tiny, Small, Medium, Large v3 Turbo,
  and Large v3 q5; downloaded/selected state is derived from the filesystem and config.
- `download_local_model` downloads the selected model, installs `whisper.cpp` from the
  pinned Windows release when needed, switches provider to `local`, and persists the paths.
- Added `dictation.delete_local_model` and AI UI deletion for downloaded local models.
- Added Settings → About storage summary:
  - total data footprint;
  - data dir and Electron userData dir;
  - ARK DB, search indexes, dictation models/tools, extensions, extension data, logs,
    crashes, and other data.
- Added headless e2e coverage for local provider path, dictation pill recording
  with a fake microphone, and AI settings visibility.
- Replaced the dictation pending queue's custom action buttons with
  `@kosmos/visuals` `Button`/settings rows.

## Handy Import Decision

Handy's embedded `transcribe-rs` / `whisper-rs-sys` path was tested as a direct
dependency, but the Windows build failed because `libclang.dll` was not available:

```text
Unable to find libclang ... set the LIBCLANG_PATH environment variable
```

To keep Kosmos buildable without a new native toolchain prerequisite, the working MVP
uses an external `whisper.cpp` executable path instead of embedding `transcribe-rs`
inside `kepler-backend`. This still provides a real local-model transcription path:
users provide both the local model file and the local CLI executable.

The follow-up Handy-like model manager uses Handy's model catalog/storage behavior as a
reference, but keeps ownership in Kosmos runtime. Handy stores models in app data under
`models`; Kosmos stores dictation models under its own app-data `models/dictation` folder.

## Checks

- `rtk cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation -- --test-threads=1`
  - `151 passed, 3 ignored, 203 filtered out`
- `rtk bunx tsc -p platform/desktop/tsconfig.json --noEmit --incremental false`
  - passed
- `rtk cargo check --manifest-path platform/runtime/Cargo.toml --lib`
  - passed, latest run: `24.24s`
- `rtk cargo fmt --manifest-path platform/runtime/Cargo.toml`
  - passed
- `rtk bunx vite build --config vite.config.mjs` from `platform/desktop`
  - passed, latest incremental run: `20.86s` renderer + `1.43s` main + `35ms` preload
  - earlier renderer/electron build after larger changes: ~1m22s renderer + ~5s electron bundles
- Download robustness/progress follow-up:
  - `download_local_model` no longer buffers the whole response through `response.bytes()`.
  - Model and whisper.cpp tool downloads now stream chunks to `.part` files with
    `Accept-Encoding: identity`, explicit HTTP/content-type diagnostics, and rejection of
    HTML/JSON/text responses before writing them as models.
  - The download path now uses `network::build_download_client` instead of the normal
    dictation API client, because the normal client has a 20s whole-request timeout for
    Groq/API calls. Large model downloads keep a connect timeout but no whole-body timeout.
  - Interrupted streams resume from the existing `.part` file via `Range: bytes=<offset>-`.
    If a server ignores range and returns `200`, the partial file is discarded and the
    download restarts cleanly.
  - `dictation.download_local_model` now starts a background backend job and returns
    immediately with `{ started: true, modelId }`, avoiding the ArkClient 30s request timeout
    for large downloads. Completion/failure/config updates are delivered through events.
  - Runtime emits `dictation_local_model_download_started/progress/complete/failed`.
  - Desktop main/preload forwards generic Ark events to settings renderers with reconnect cleanup.
  - AI settings creates an optimistic progress state immediately on click, updates from backend
    events, disables duplicate actions while a model is downloading, and shows a visible row
    progress bar plus percent/bytes label.
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-dictation'; cargo check --manifest-path platform/runtime/Cargo.toml --lib"`
  - passed in isolated target after streaming download changes: `4m 06s`
- `rtk bunx tsc -p platform/desktop/tsconfig.json --noEmit --incremental false`
  - passed after progress UI/event typing changes
- `rtk cargo fmt --manifest-path platform/runtime/Cargo.toml`
  - passed after streaming download changes
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-dictation'; cargo test --manifest-path platform/runtime/Cargo.toml --lib local_models::tests -- --test-threads=1"`
  - covers chunked binary download progress, rejects `text/html` responses before writing model files,
    and resumes an existing `.part` file with a `Range` request.
  - `3 passed; 0 failed; 358 filtered out`, latest run: `7.19s` compile + `0.03s` tests
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-dictation'; cargo test --manifest-path platform/runtime/Cargo.toml --lib network::tests::build_client_system_profile -- --test-threads=1"`
  - passed after adding `build_download_client`: `1 passed; 0 failed; 360 filtered out`
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-dictation'; cargo test --manifest-path platform/runtime/Cargo.toml --lib local_models_list_and_use_downloaded_model_updates_config -- --test-threads=1"`
  - passed after streaming download changes: `1 passed; 0 failed; 357 filtered out`
  - build/test wall time in isolated test profile: `1m 53s`
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-dictation'; cargo check --manifest-path platform/runtime/Cargo.toml --lib"`
  - passed after job-style download change: latest run `3m 34s` (included package/build lock wait from a parallel cargo test)
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-dictation'; cargo build --manifest-path Cargo.toml --bin kepler-backend"`
  - passed after job-style download change: `2m 04s`
- `rtk bunx tsc -p platform/desktop/tsconfig.json --noEmit --incremental false`
  - passed after progress UI job-state changes
- `rtk bunx vite build --config vite.config.mjs` from `platform/desktop`
  - passed after progress UI job-state changes: `24.99s` renderer + `1.29s` main + `20ms` preload
- `rtk bunx vite build --config vite.config.mjs` from `platform/desktop`
  - passed after final event cleanup patch: `1.86s` renderer + `2.38s` main + `16ms` preload
  - previous cold-ish run in the same turn: `3m 46s` renderer + `11.58s` main + `18ms` preload
- `rtk cargo build --manifest-path platform/runtime/Cargo.toml --bin kepler-backend`
  - passed, fresh Electron backend binary built
  - latest warm build: ~17s; earlier colder build after backend changes: ~1m09s
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-dictation'; cargo build --manifest-path Cargo.toml --bin kepler-backend"`
  - passed in isolated target after Windows target/debug locks; cold build: `5m 10s`
- Electron runtime probe with `KEPLER_BACKEND_EXE=.tmp/cargo-dictation/debug/kepler-backend.exe`
  - `dictation.list_local_models` returned `modelCount: 5`, first model `Whisper Tiny`
- `rtk npx playwright test --config platform/desktop/playwright.config.ts --reporter=line --output .tmp/playwright-ai-model-catalog platform/desktop/e2e/dictation.spec.ts --grep 'advanced AI page shows local dictation model settings'`
  - with `KEPLER_BACKEND_EXE=.tmp/cargo-dictation/debug/kepler-backend.exe`
  - `PASS (1) FAIL (0)`, latest run: `33536ms`
- `rtk npx playwright test --config platform/desktop/playwright.config.ts --reporter=line --output .tmp/playwright-settings-about platform/desktop/e2e/settings-about.spec.ts`
  - `PASS (1) FAIL (0)`, latest run: `19819ms`
- `rtk npx playwright test --config platform/desktop/playwright.config.ts --reporter=line --output .tmp/playwright-dictation-local-ai platform/desktop/e2e/dictation.spec.ts`
  - `PASS (10) FAIL (0) skipped (2)`, latest run: `48454ms`
- `rtk npx playwright test --config platform/desktop/playwright.config.ts --reporter=line --output .tmp/playwright-dictation-local-pill platform/desktop/e2e/dictation.spec.ts -g "local provider records through dictation pill with fake microphone"`
  - Covers `kepler:dictation` start -> hidden pill `getUserMedia` fake microphone ->
    stop -> renderer WAV encode -> `dictation.submit_audio` -> local provider ->
    clipboard-only injection.
  - `PASS (1) FAIL (0)`, latest run: `9138ms`
- Real local smoke setup under `sample/local-stt`:
  - downloaded `whisper-bin-x64.zip` from whisper.cpp release `v1.8.4`
  - downloaded `ggml-tiny-q5_1.bin` from Hugging Face `ggerganov/whisper.cpp`
  - generated `kosmos-test.wav` through Windows SAPI
  - direct `whisper-cli.exe -m ggml-tiny-q5_1.bin -f kosmos-test.wav -l en -otxt ...`
    returned a transcript containing `Hello`
- `rtk npx playwright test --config platform/desktop/playwright.config.ts --reporter=line --output .tmp/playwright-dictation-local-real platform/desktop/e2e/dictation.spec.ts -g 'opt-in real local whisper.cpp provider works headless without microphone'`
  - with `KOSMOS_TEST_LOCAL_DICTATION_COMMAND_PATH`, `KOSMOS_TEST_LOCAL_DICTATION_MODEL_PATH`,
    `KOSMOS_TEST_DICTATION_AUDIO_PATH`, and expected substring `hello`
  - `PASS (1) FAIL (0)`, latest run: `6389ms`
- Local whisper.cpp transcript cleanup / latency follow-up:
  - `platform/runtime/src/dictation/local.rs` now prefers the generated `.txt` transcript over
    stdout and strips whisper-style timestamp prefixes defensively.
  - whisper.cpp is called with `-nt`, `-np`, thread count via `KOSMOS_LOCAL_WHISPER_THREADS`
    or available CPU parallelism capped to 8, plus `-bo 1 -bs 1` for short dictation latency.
  - Handy comparison confirmed the main remaining latency gap is architectural: Handy keeps a
    `LoadedEngine::Whisper(WhisperEngine)` in memory and reuses it, while Kosmos currently starts
    external `whisper-cli.exe` and reloads the model for every dictation.
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-local-fix'; cargo check --manifest-path platform/runtime/Cargo.toml --lib"`
  - passed after local whisper cleanup/flags: `1m 52s` in fresh target
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-local-fix'; cargo build --manifest-path Cargo.toml --bin kepler-backend"`
  - passed after local whisper cleanup/flags: `2m 45s` in fresh target
- `rtk proxy powershell -NoProfile -Command "$env:CARGO_TARGET_DIR='.tmp\cargo-dictation'; cargo test --manifest-path platform/runtime/Cargo.toml --lib local::tests -- --test-threads=1"`
  - interrupted after repeated long silence in Windows test profile while compiling `ark-core`; follow-up
    `cargo check --lib` and `cargo build --bin kepler-backend` passed in a fresh target.
- Visual verification:
  - `.tmp/visual/2026-06-19-local-dictation-ai-settings/settings-ai-local-capture-debug.png`
    shows the advanced `AI` settings page selected in the shared settings shell.
  - `.tmp/visual/2026-06-19-local-dictation-ai-settings/settings-ai-local-fields-capture-debug.png`
    shows local model fields populated from runtime config:
    `C:/models/whisper-test.bin`, `C:/tools/whisper-cli.exe`, `whisper-test`,
    `Whisper.cpp`, and active local status.
  - DOM/capture debug confirmed the captured Electron `#settings` window body included
    the local AI settings content before the PNG was written.

## Interrupted / Not Re-run

- A repeat `rtk cargo test --manifest-path platform/runtime/Cargo.toml --lib local_models_list_and_use_downloaded_model_updates_config -- --test-threads=1`
  was stopped after `rustc` sat on the same `ark_core` compile for roughly 10 minutes
  with near-zero CPU in the shared Windows `target/debug` dir. The exact test had passed
  earlier before the later e2e-only timeout tweak, and the isolated backend build passed.

## Residual Manual Checks

- Physical microphone capture + local model latency on Windows.
- macOS local CLI path / packaging check.
- Cancel/resume UX for large local model downloads.
