# Desloppify Delphi Live Voice PCM Name Cleanup

## Classification

FULL_LOOP. This is a narrow production helper rename inside the ongoing desloppify proof loop.

## Goal

Remove the `NUMERIC_SUFFIX` finding in `products/delphi/src/services/gemini/liveVoiceTasks.ts` without changing audio conversion behavior.

## Change

- Renamed file-local helper `floatToPcm16` to `floatToPcmBytes`.
- Kept the Int16 PCM conversion, output bytes, and call site behavior unchanged.

## Verification

- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS: `rtk err bun run --cwd platform/desktop build:extension delphi`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-live-voice-pcm-name-cleanup.json"`

The scan exits 1 because repository findings remain, but the targeted finding disappeared.
