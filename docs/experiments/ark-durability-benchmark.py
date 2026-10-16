#!/usr/bin/env python3
"""Compare WAL NORMAL and FULL commit latency with identical SQLite writes."""

import os
import sqlite3
import statistics
import tempfile
import time


TRIALS = 5
WRITES_PER_TRIAL = 700
PAYLOAD_BYTES = 512


def measure(mode: str, trial: int) -> float:
    with tempfile.TemporaryDirectory(prefix=f"ark-sync-{mode.lower()}-") as folder:
        path = os.path.join(folder, "bench.db")
        conn = sqlite3.connect(path)
        conn.execute("PRAGMA journal_mode=WAL")
        conn.execute(f"PRAGMA synchronous={mode}")
        conn.execute("CREATE TABLE writes (id INTEGER PRIMARY KEY, body BLOB NOT NULL)")
        conn.execute("INSERT INTO writes VALUES (1, zeroblob(?))", (PAYLOAD_BYTES,))
        conn.commit()
        start = time.perf_counter()
        for index in range(WRITES_PER_TRIAL):
            conn.execute("BEGIN IMMEDIATE")
            body = bytes([index % 256]) * PAYLOAD_BYTES
            conn.execute("UPDATE writes SET body=? WHERE id=1", (body,))
            conn.commit()
        elapsed = time.perf_counter() - start
        row_count, body, body_len = conn.execute(
            "SELECT COUNT(*), body, length(body) FROM writes WHERE id=1"
        ).fetchone()
        expected_body = bytes([(WRITES_PER_TRIAL - 1) % 256]) * PAYLOAD_BYTES
        assert row_count == 1 and body_len == PAYLOAD_BYTES and body == expected_body
        conn.close()
        return elapsed * 1000 / WRITES_PER_TRIAL


def main() -> None:
    samples = {"NORMAL": [], "FULL": []}
    for trial in range(TRIALS):
        order = ("NORMAL", "FULL") if trial % 2 == 0 else ("FULL", "NORMAL")
        for mode in order:
            samples[mode].append(measure(mode, trial))
    for mode, values in samples.items():
        print(
            f"{mode}: per-commit ms trials={[round(x, 4) for x in values]} "
            f"median={statistics.median(values):.4f} "
            f"range={min(values):.4f}-{max(values):.4f}"
        )
    print(
        f"Each trial committed {WRITES_PER_TRIAL} WAL transactions and verified "
        f"one {PAYLOAD_BYTES}-byte row; {TRIALS} alternating trials per mode."
    )


if __name__ == "__main__":
    main()
