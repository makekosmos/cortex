# ADR-001: Use FULL synchronous WAL commits for ARK

- Status: Accepted for the primary user database
- Date: 2026-10-16
- Scope: SQLite transaction durability for the local ARK database

## Context

KOS-420 adds a committed feed and durable inbox receipts. The storage command
must not report a successful receipt before its SQLite transaction has committed
with the durability level required by the feed. `NORMAL` is quicker in the local
microbenchmark, but can lose recently committed transactions after an operating
system or power failure.

## Decision

Open the primary user database in WAL mode with `synchronous=FULL`. A successful
database command receipt therefore follows a FULL-synchronous WAL commit,
conditional on the operating system, storage device, and hardware honoring flush
requests. The setting does not protect against dishonest hardware, media failure,
or restoring an older backup.

The setting applies to SQLite commit acknowledgment. It does not promise
exactly-once transport delivery, nor does it replace inbox replay, backup
validation, or restore recovery.

## Evidence and limits

The attached [microbenchmark](../experiments/2026-10-16-ark-sync-durability.md)
ran five alternating trials of 700 separate WAL transactions per setting. It
observed median per-commit times of 0.0578 ms for FULL and 0.0154 ms for NORMAL
on the measured host. The script verified the final payload byte-for-byte, but
did not identify the timing limiter, profile flush syscalls, or measure
production Engine requests. Its performance verdict is inconclusive and does
not establish a supported-device latency bound; it is not the basis for the
durability decision.

## Consequences

- Callers receive success only after the selected SQLite commit has completed.
- FULL incurs additional flush work; production p99 must be measured on
  supported Windows and macOS devices.
- A future change to NORMAL requires new durability evidence and an explicit
  change to what a successful receipt means.
