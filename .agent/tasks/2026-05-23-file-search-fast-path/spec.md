# File Search Fast Path

## Goal

Make Kepler File Search feel responsive while the current fallback index remains
available, and add a Windows NTFS fast initial scan path for fixed local drives.

## Context

- File Search v1 already indexes files into host-local `file-index.db` and
  surfaces filename/path results in the launcher.
- On a large index, a launcher query can spend visible time in backend search.
  The launcher currently renders an empty state while the request is pending.
- The current scanner walks the filesystem. On local NTFS volumes Windows
  exposes MFT enumeration through USN control codes; non-NTFS or unavailable
  fast scanning must keep using the existing fallback walk.

## Scope

### In

- Launcher feedback for an in-flight file search request.
- Backend search-store changes that avoid the large-table slow path for normal
  filename/path queries where the SQLite index can serve them.
- Windows NTFS fast initial scanning for fixed-drive roots, with fallback to the
  existing walk scanner when the fast path is unavailable.
- Focused automated coverage for store search behavior and scanner fallback
  contracts plus the existing backend/shell verification commands.

### Out

- Content search inside files.
- Folder results in the launcher.
- Network shares and removable drives.
- A privileged helper/service or automatic UAC elevation flow.
- Replacing the existing filesystem watcher with USN journal live updates.

## Acceptance Criteria

**AC1.** While a non-empty File Search request is pending, the launcher shows a
Russian-language file-search progress state instead of presenting only the
empty-result state.

**AC2.** File Search backend uses an indexed SQLite search path for normal
filename/path queries and preserves filename/path result semantics for existing
search tests.

**AC3.** On Windows local NTFS fixed-drive roots, File Search can use an NTFS
MFT/USN-based initial scanner; when that scanner cannot enumerate a root, the
existing filesystem walk still indexes the root.

**AC4.** Verification is green for touched backend and shell surfaces:
focused file-index tests, `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`,
`bun run --cwd shell typecheck`, `bun run --cwd shell build:js`,
`bun run ark:guard:writes`, and `bun run ark:smoke`.
