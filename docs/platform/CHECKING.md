# Checking the Engine platform matrix

`features.json` lists what the Engine and ARK can do on each OS and how the OS code is shaped. It tracks the work toward one Engine for every platform. A daily routine checks it against the code and opens a PR when something changed. A person can follow the same steps.

## Sources

- `runtime/` (Engine) and `core/crates/ark-core/` (ARK) on `main` of `makekosmos/cortex`. This is an engineering matrix, so check `main`, not the latest release.
- Release build scripts decide what ships: `desktop/scripts/build-backend.mjs` (Windows) and `desktop/scripts/build-macos-native.mjs` (macOS). A capability behind a Cargo feature that a release script does not enable is `no` on that OS, with a note.
- Manager and the apps are out of scope. List only what the Engine or ARK offers.

## Statuses

- `yes`: a real implementation exists for that OS and ships in its release build.
- `partial`: it works with a limit. The `note` names the limit in a few words.
- `no`: not implemented, or a stub (`Err`, `Ok(())` that does nothing, `unsupported-platform`). Say which in the `note` when it is a stub.
- `n/a`: the OS has no such concept or forbids it (a tray on Android, a global hotkey on iOS).
- `unverified`: a code path exists but nothing builds or runs it for that OS.

Linux, Android and iOS have no release build yet. For them, `yes` means the code path is the same portable code that ships on Windows and macOS.

## Boundary

How the OS-specific code is shaped. This is the measure of progress toward one Engine.

- `portable`: no OS code.
- `seam`: one function or trait dispatches per OS, and callers stay free of `cfg`.
- `scattered`: `cfg` gates sit in callers, struct fields or many files, with no single seam.

The goal is every feature at `portable` or `seam`. Moving a feature from `scattered` to `seam` is a change worth recording even when no status changes.

## Steps

1. Check out `main`.
2. For every feature, confirm each status and the boundary from the code: follow the `code` anchors (`path` or `path#symbol`) and read the `cfg(...)` gates around them. Read the code before changing a status. Do not judge by file names.
3. Confirm every `code` anchor still exists. When code moved, update the anchor.
4. Read the commits since `source` that touch `runtime/` or `core/`. Add a feature only when it is a new Engine capability that has OS-specific code or could have it. Put it in the matching group with a short sentence-case English name.
5. Remove a feature only when its code is gone on every platform.
6. Set `checked` to today's date and `source` to the short SHA you checked.

If only `checked` and `source` would change, change nothing. Otherwise open a PR from a branch named `platform-YYYY-MM-DD`. The PR body lists each change with the file and line that proves it. Never merge it.
