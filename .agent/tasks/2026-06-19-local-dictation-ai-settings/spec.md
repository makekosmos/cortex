# Local Dictation + AI Settings

## Goal

Implement a working local-model transcription path for Kosmos dictation and expose AI
provider/model settings in Settings advanced UI using only existing `@kosmos/visuals`
settings primitives.

## Scope

- Runtime dictation supports at least `provider = "groq"`, `provider = "mock"`, and
  `provider = "local"`.
- Local provider can transcribe submitted dictation audio through a local Whisper-style
  model path, without requiring Groq API key.
- Dictation keeps the existing recording -> submit_audio -> transcript -> inject state
  machine, pending queue, stats, and retry behavior.
- Settings gains an advanced `AI` page for online/local transcription provider setup.
- Dictation settings can choose local or online transcription method.
- AI settings exposes a Handy-like local model catalog: the user can choose a model,
  download it into Kosmos app data, install the local whisper.cpp runner on Windows, and
  auto-select local dictation without typing full paths.
- UI must use existing `@kosmos/visuals` settings rows/buttons/dropdowns/toggles/inputs;
  no new custom cards/buttons/toggles for this feature.

## MVP Decisions

- Use Handy as a reference, not as a wholesale import.
- Start with Whisper-only local STT.
- Implement the minimal model catalog/download flow directly in Kosmos runtime instead of
  importing all of Handy.
- Keep manual model/command path fields as an advanced fallback.
- Keep renderer WAV submission as the audio input path for MVP.
- Add test-only local transcription hooks where required for deterministic CI/headless
  coverage, but keep the production local path wired behind the same provider dispatch.

## Acceptance Criteria

1. `dictation.update_config` can set `provider = "local"` and local model fields.
2. `dictation.get_config` exposes local model readiness/settings enough for UI.
3. `dictation.submit_audio` with `provider = "local"` succeeds in a deterministic test
   path and returns to `idle` without using Groq key/network.
4. Existing Groq and mock dictation tests still pass.
5. Advanced Settings contains an `AI` page built from `@kosmos/visuals` components.
6. Dictation Settings exposes a local/online provider choice and persists it.
7. Automated verification covers runtime local dispatch and UI/provider selection.
8. Local model catalog can list available Whisper models, mark downloaded/selected state
   from files, download a selected model, install whisper.cpp on Windows, and persist the
   selected local provider config.

## Out Of Scope For This Pass

- Shipping bundled large model binaries.
- Progress/cancel/resume UI for model downloads.
- Replacing renderer audio capture with native VAD/recorder.
- macOS real-device local model performance verification.
