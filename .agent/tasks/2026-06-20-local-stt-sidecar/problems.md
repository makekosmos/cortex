# 2026-06-20 local-stt-sidecar problems

No open acceptance-criteria problems.

All ACs are marked `PASS` in `evidence.md` / `evidence.json`.

Resolved blockers:

- AC2 moved from `PARTIAL` to `PASS` after adding the dynamic `whisper.dll`
  sidecar backend. The main runtime still does not link native Whisper.
- AC6 moved from unit-only proof to full proof after
  `raw/sidecar-crash-smoke.ps1` killed the sidecar during active transcription,
  verified the pending WAV remained, and verified a fresh sidecar could answer
  `status`.
- Broad `dictation` Rust tests now pass: `178 passed, 3 ignored, 217 filtered
out`.
- Desktop dictation e2e now passes: `10 passed, 2 skipped`.
- `ark:guard:writes` now passes after the pre-existing
  `settings-window.ts` user-data path access was routed through
  `resolveInstance().userDataDir`.
