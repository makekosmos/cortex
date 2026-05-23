# File Search NTFS Reader

## Problem

Kepler file search has a hand-written Windows NTFS fast path. It is fragile and
hard to maintain, while the app already expects the backend to run elevated for
fast local-drive indexing.

## Acceptance Criteria

1. Windows drive-root scans use the `ntfs-reader` crate to read the MFT instead
   of the local `FSCTL_ENUM_USN_DATA` parser.
2. File index output keeps normal user-facing paths like `C:\Users\...`, not
   NT device paths.
3. Existing noisy-folder exclusion behavior remains intact for NTFS fast scans.
4. Existing file-index Rust tests pass.
5. Backend library tests pass.
