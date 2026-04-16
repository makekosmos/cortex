# Performance Benchmarks (Performance Pass)

Command:
`cargo test perf_bench -- --ignored --nocapture` (run in `src-tauri`)

## Datasets
- Library load: in-memory SQLite with 5,000 games (id, name, exe_path, exe_name, date_added).
- Scan: 10 dirs × 100 `.exe` + 100 `.txt` files (1,000 executables).
- Backup: 20 dirs × 20 files, 8KB each (3.2MB total), directory mode, 4 threads.

## Results (ms)

| Metric | Baseline | After | Notes |
| --- | --- | --- | --- |
| Library load (5,000 rows) | 134 | 102 | ~24% faster (index + schema tweaks). |
| Scan executables (1,000 exe) | 2 | 2 | Stable. |
| Backup create (3.2MB) | 146 | 136 | Slight improvement. |
| Backup restore (3.2MB) | 119 | 120 | Within noise. |

Notes:
- Timings are from a single local run; expect variance on different disks/CPUs.
- Further speedups are likely from reducing IO and batching game inserts during scan.
