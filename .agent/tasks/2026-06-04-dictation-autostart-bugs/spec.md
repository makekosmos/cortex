# 2026-06-04 — Dictation >30s and autostart false error

## Goal

Fix two reported production bugs:

- Settings shows "Не удалось применить настройку" after enabling Windows autostart even when the registry change actually succeeds.
- Dictation recordings longer than about 30 seconds lose trailing speech and only the main/early part is transcribed.

## Scope

- In scope: Settings autostart toggle feedback, dictation Groq STT submission for long recordings, regression tests, bug postmortem.
- Out of scope: changing release/version numbers, adding local Whisper, changing dictation UI visuals, changing Windows installer behavior.

## Acceptance Criteria

**AC1.** Enabling/disabling autostart no longer shows "Не удалось применить настройку" solely because an immediate readback returns the previous value after `set` did not throw.

**AC2.** Dictation audio longer than 30 seconds is sent to Groq as ordered <=30 second WAV chunks and concatenated, so speech after the first 30 seconds is preserved.

**AC3.** Regression tests cover both the autostart UI readback race and WAV chunking/transcription concatenation behavior.

**AC4.** `docs-site/agents/postmortems.md` contains a pre-fix bug entry and is completed after the fix with root cause, fix, regression protection, and prevention.
