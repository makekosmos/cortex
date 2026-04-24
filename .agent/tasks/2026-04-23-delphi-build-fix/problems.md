# Problems

## Current Blocker

`bun run build` now reaches the Windows packaging stage, but `electron-builder` fails to spawn:

- `D:\Personal\Hobby\Coding\kepler\node_modules\.bun\app-builder-bin@5.0.0-alpha.12\node_modules\app-builder-bin\win\x64\app-builder.exe`
- Error: `spawn EPERM`

## Interpretation

This is no longer the original dependency-resolution failure. The prior `ajv` / `ajv-keywords` crash is resolved, and the default build script no longer requests an invalid macOS build on Windows.

It is also no longer the `winCodeSign` symlink-extraction failure from the user's log. That path was removed by:

- switching Windows packaging off the legacy `winCodeSign-2.6.0.7z` path
- disabling `signAndEditExecutable` for the unsigned Windows build
- fixing the packaged sidecar resource path

The remaining failure happens when the sandboxed verification environment tries to execute `app-builder.exe`. That indicates an execution-permission restriction in the current environment rather than a JavaScript dependency mismatch in Delphi itself.

## Smallest Safe Next Step

Re-run `bun run build` in a normal local shell outside this sandboxed tool environment. If `app-builder.exe` still fails there, capture that exact error separately because it is a different issue from the original one.
