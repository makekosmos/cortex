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
- For faster-whisper regressions, build `kosmos-local-stt.exe` into a separate
  `CARGO_TARGET_DIR`, then smoke it as a real stdio sidecar with `KOSMOS_DATA_DIR`
  pointing at the dev data dir and without `KOSMOS_FASTER_WHISPER_DLL_DIRS`,
  `KOSMOS_FASTER_WHISPER_PYTHON`, or `KOSMOS_LOCAL_STT_DIR`. Send `preload` and
  `transcribe` requests against a generated WAV; a valid smoke should report
  `backend=faster_whisper`, `accelerator=gpu` on NVIDIA machines, and a non-empty
  transcript.
- If dev works but the installed build fails immediately, compare the packaged
  `extraResources` name with runtime lookup. The installer ships `Kosmos Local STT.exe`,
  while dev builds use `kosmos-local-stt.exe`; runtime must accept both names.

## Avoid

- Do not make `transcribe-rs` a required dependency of `kepler-backend` unless CI and
  developer Windows machines have `LIBCLANG_PATH` configured.
- Do not call a deterministic test transcript path a real local-model smoke.
- Do not rely on `whisper.cpp-cublas/Release` as a hidden source of faster-whisper
  CUDA DLLs; the managed faster-whisper venv should provide its own runtime DLLs.
- Do not "fix" `cublas64_12.dll is not found` by making faster-whisper Auto always
  CPU. On NVIDIA Windows machines, model preparation should install the managed
  CUDA runtime or mark the model unprepared.
- Do not validate local STT only through `bun run --cwd platform/desktop dev`;
  packaged resource names are different enough to need a release-folder smoke or
  a unit test for candidate path generation.

## Promote To Skill When

Promote this if local AI/model work becomes frequent across dictation, Eden, or other
Kosmos AI features.
