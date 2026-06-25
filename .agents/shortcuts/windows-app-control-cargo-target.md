# Windows App Control Cargo Target

## Trigger

When Cargo on Windows reports impossible dependency errors after interrupted builds, especially `can't find crate for paste`, `uniffi`, `scroll_derive`, or build scripts failing with os error 4551.

## Symptom

Rust code and `cargo tree` show the dependency is present, but `cargo build` fails as if a proc-macro crate is missing. Fresh `CARGO_TARGET_DIR` under `.tmp` may fail with:

```text
An Application Control policy has blocked this file. (os error 4551)
```

## Do This

- Treat this as a Windows Application Control / target artifact issue before editing source.
- Check the exact failure with `cargo build -vv` and look for blocked build scripts or proc-macro DLLs.
- For desktop release builds, prefer an absolute repo-root target dir:

```powershell
rtk proxy cmd /C "set CARGO_TARGET_DIR=C:\Users\kirill\Coding\kosmos\.tmp\cargo-release&& bun run --cwd platform/desktop build:backend"
```

- If `electron-builder` still expects `../../target/release/*.exe`, copy the five release binaries from the clean target into `target\release` before running `node platform/desktop/scripts/build-desktop.mjs --platform win`.
- Prefer an already allowed target path when you need to run a binary, for example an existing `target\\release` executable.
- If a freshly built debug binary is blocked, verify whether an older signed/allowed release binary can run the same runtime path before spending time on rebuilds.
- Use direct sidecar preload/transcribe probes to validate STT runtime behavior independently from Electron.

## Avoid

- Do not patch third-party crates or project manifests just because rustc reports `can't find crate` for a dependency that exists in `cargo tree`.
- Do not use a relative `CARGO_TARGET_DIR=.tmp\...` from `platform/desktop`; it resolves under `platform/desktop\.tmp` and can hit os error 4551.
- Do not assume `Unblock-File` fixes policy-blocked generated binaries.

## Promote To Skill When

This appears outside STT work or becomes part of the normal Windows dev setup checklist.
