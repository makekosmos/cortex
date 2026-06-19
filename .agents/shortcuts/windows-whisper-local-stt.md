# Windows Whisper Local STT

## Trigger

Use this when adding or debugging local Whisper/STT support in Kosmos on Windows.

## Symptom

Adding embedded `transcribe-rs` / `whisper-rs-sys` fails during Cargo build with:

```text
Unable to find libclang ... set the LIBCLANG_PATH environment variable
```

## Do This

- Keep embedded Whisper optional until the Windows toolchain/packaging story includes
  `libclang.dll`.
- For an MVP that must work now, use an external `whisper.cpp` executable adapter:
  user-configured `whisper-cli.exe` path + user-configured ggml model path.
- Verify with a small model under `sample/`, for example `ggml-tiny-q5_1.bin`, and a
  generated WAV before claiming the local path works.

## Avoid

- Do not make `transcribe-rs` a required dependency of `kepler-backend` unless CI and
  developer Windows machines have `LIBCLANG_PATH` configured.
- Do not call a deterministic test transcript path a real local-model smoke.

## Promote To Skill When

Promote this if local AI/model work becomes frequent across dictation, Eden, or other
Kosmos AI features.
