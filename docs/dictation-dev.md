# Dictation local development

How the Engine + dictation-gpui dev loop is wired on macOS. Verified
2026-10-07.

## Processes

- Engine watcher (from `cortex/`):
  `mbx watch -E MUNDUS_SKIP_SYNC=1 -x "run -p engine --features local-dictation --bin mundus-engine"`
  (spawns `mundus-engine` plus a `--core-worker` child; log `/tmp/engine-watch.log`).
- App watcher (from `dictation/gpui/`):
  `mbx watch -E MUNDUS_DATA_DIR=/Users/user/.config/Mundus -x run`
  (log `/tmp/dictation-watch.log`).

## Endpoint discovery

Both sides read `<MUNDUS_DATA_DIR>/engine.lock.json` (`http_port`, `ws_port`,
`auth_token`, pid). Engine and the app must run with the same data dir or the
app fails with `Transport: no response from Engine`. Never print the token.

After a branch switch/rebuild expect several stale `mundus-engine` processes:
kill all of them, confirm the lock file pid/ports match the fresh process,
then let the app reconnect. Check health over `http://127.0.0.1:<port>/v1/rpc`
with `Authorization: Bearer <token>` + `x-kosmos-client-pid` + `x-kosmos-api-version`
headers.

## Native helpers are not rebuilt by cargo-watch

`runtime/native/macos/*.swift` (audio-capturer, paste-text, …) compile via
`node desktop/scripts/build-macos-native.mjs` and are copied into
`target/<profile>/native/macos/`. `mbx watch` does not rebuild them, and stale
binaries survive `git switch` — always rebuild after touching them or when a
helper-driven test misbehaves.

## Logging caveat

`tracing::info` output does not reach `/tmp/engine-watch.log`; `eprintln!`
does. Dictation injection diagnostics use `[dictation::inject] …` eprintln
lines for this reason.

## Testing dictation without a mic or speakers

`dictation.capture.start` accepts `sourceFile` (wav/mp3/m4a) and `sourceSpeed`
(>0, capped at 64): the helper decodes the file into the normal
ring/meter/drain/streaming path instead of the mic. File-source captures emit
no `dictation_audio_level` broadcasts, so the pill never shows test runs.
`dictation.speech.transcribe` accepts `delivery: "text_only"` — no paste and
no clipboard write. `scripts/dictation-file-e2e.mjs <file> [--speed N]` drives
the whole pipeline.

## lefthook environment

Hooks stash unstaged changes — stage every touched file before committing, or
the gate checks a stale tree. Gate binaries (oxfmt, oxlint, lefthook itself)
live on pnpm's PATH: invoke them via `pnpm exec`, not bare.
