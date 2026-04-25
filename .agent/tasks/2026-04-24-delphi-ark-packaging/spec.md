# Delphi ARK Packaging Fix

## Context

Delphi's Electron package must ship the canonical ARK runtime binary, `ark-core-rpc`, from `packages/ark-core/rust`.

The current Delphi package configuration builds the legacy Delphi sidecar from `apps/delphi/ts/sidecar/Cargo.toml` and copies `delphi-db.exe` into the packaged app as `ark-core/ark-core-rpc.exe`. Runtime Delphi code expects ARK operations provided by the real `ark-core-rpc`, so the packaged app can receive an incompatible binary.

## Scope

Make the smallest packaging change necessary so Delphi builds and packages the canonical ARK sidecar.

## Acceptance Criteria

- AC1: `apps/delphi/ts/package.json` release sidecar build script builds `packages/ark-core/rust/Cargo.toml` with `--bin ark-core-rpc`.
- AC2: `apps/delphi/ts/package.json` dev sidecar build script builds the same canonical `ark-core-rpc` binary.
- AC3: Electron Builder `extraResources` copies `packages/ark-core/rust/target/release/ark-core-rpc.exe` to `ark-core/ark-core-rpc.exe` and no longer copies or renames `apps/delphi/ts/sidecar/target/release/delphi-db.exe`.
- AC4: A fresh verification pass records current file evidence and command results in this task directory.

## Out Of Scope

- Removing the legacy Delphi sidecar source.
- Refactoring the shared ARK Node SDK.
- Changing runtime sidecar protocol behavior.
- Building full Delphi installer artifacts unless needed for verification.
