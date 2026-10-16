# ARK WAL durability comparison

This experiment supports [ADR-001](../adr/ADR-001-ark-wal-full.md). It is a
local SQLite microbenchmark, not an end-to-end sync latency result.

## Method

Python 3.9.6 `sqlite3` on macOS, temporary local database per run, WAL mode,
one 512-byte row updated in 700 separate `BEGIN IMMEDIATE`/`COMMIT`
transactions. The script verified the row count and payload size after every
trial. The final row body was checked byte-for-byte against the expected last
write. Five trials per setting ran in alternating order. At start, system load
was 2.67 over 8 logical CPUs. Both settings used the same SQL and SQLite
defaults; no concurrent synthetic workload was added.

Reproduce with `python3 docs/experiments/ark-durability-benchmark.py`.

## Results

| `synchronous` | Per-commit ms, five alternating trials | Median | Range |
|---|---|---:|---:|
| `NORMAL` | 0.0176, 0.0154, 0.0152, 0.0155, 0.0152 | 0.0154 | 0.0152–0.0176 |
| `FULL` | 0.0594, 0.0618, 0.0520, 0.0550, 0.0578 | 0.0578 | 0.0520–0.0618 |

Every trial completed all 700 commits and verified the expected final
512-byte body. The performance verdict is inconclusive: this script does not
identify the limiter, profile SQLite flush syscalls, or measure a production
Engine request. Its numbers do not support an end-to-end latency comparison,
device limit, or sync responsiveness claim. The durability choice is based on
commit guarantees, not this timing result. Storage flush latency and user-facing
p99 remain to be measured on supported Windows/macOS devices.
