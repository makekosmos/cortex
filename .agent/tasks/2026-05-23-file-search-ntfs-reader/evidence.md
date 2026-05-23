# Evidence

## AC1 - `ntfs-reader` MFT scan

PASS

- `services/kepler-backend/src/file_index/scanner/ntfs.rs` now uses
  `ntfs_reader::volume::Volume`, `ntfs_reader::mft::Mft`, and
  `FileInfo::with_cache` for drive-root scans.
- The previous local `FSCTL_ENUM_USN_DATA` parser was removed.

## AC2 - Normal paths

PASS

- `user_path` converts `\\.\C:\...` and `\\?\C:\...` paths to `C:\...`.
- Covered by `file_index::scanner::ntfs::tests::converts_nt_device_paths_to_user_paths`.

## AC3 - Noisy-folder filtering

PASS

- NTFS fast scans still call `path_contains_noisy_folder` before inserting
  `IndexedFile` entries.

## AC4 - File-index tests

PASS

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index
```

Result: `7 passed; 0 failed`.

## AC5 - Backend library tests

PASS

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib
```

Result: `158 passed; 0 failed`.
