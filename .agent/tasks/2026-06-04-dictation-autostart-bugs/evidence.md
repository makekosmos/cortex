# Evidence — 2026-06-04 dictation >30s + autostart false error

## Fixed

- Settings autostart toggle:
  - Successful `autostart.set(desired)` no longer immediately re-reads Windows autorun state and reports a false "Не удалось применить настройку".
  - Real write failures still show "Ошибка записи в реестр" and restore the current checked state.
- Dictation long-form transcription:
  - Backend parses renderer PCM WAV and splits recordings longer than 30 seconds into ordered <=30 second WAV chunks before sending to Groq.
  - Chunk transcripts are filtered as before and concatenated in original order.
  - Unsupported WAV shapes fall back to the previous single-request path.
- Docs:
  - `docs-site/concepts/dictation.md` documents long-form chunking.
  - `docs-site/agents/postmortems.md` records root cause, fix, regression protection, and prevention.

## Verification

- `bun test tests/unit/settings-autostart-ui.test.ts` — PASS.
- `bun run ark:smoke` — PASS, including `dictation::groq::tests::split_wav_for_transcription_*` and `transcribe_posts_long_audio_in_ordered_chunks`.
