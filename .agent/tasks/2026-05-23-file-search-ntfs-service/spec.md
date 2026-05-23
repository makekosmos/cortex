# File Search NTFS Service

## Problem

`ntfs-reader` requires an elevated token. Kepler already has a privileged
Windows Service broker (`kepler-focus-svc`) used for hosts-file operations, so
file search should reuse that broker instead of elevating the whole backend.

## Acceptance Criteria

1. Shell spawns `kepler-backend.exe` normally; no backend-wide UAC path remains.
2. The service pipe protocol exposes an NTFS scan operation returning file path,
   name, and mtime records.
3. Backend NTFS drive-root scan tries the service operation first and falls back
   to local `ntfs-reader`/walk behavior when the service is unavailable.
4. Noisy-folder filtering is supported through the service path.
5. TypeScript checks and relevant Rust tests/builds pass.
