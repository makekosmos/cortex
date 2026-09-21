# Phase 9 proof rerun — 2026-09-08

Source is the immutable proof commit
`de9642fda19aa9d4284fb873b05e8024feab3203`, run in a clean detached worktree.

Command:

`cargo test --manifest-path core/ark/crates/ark-core/rust/Cargo.toml --test phase9_proof_bundle -- --nocapture`

Result: `PASS 1/1`.

Raw assertion line:

`proof_digest=c012d55bea98bfebf0dd7e8e56d56f3a900a4afb008a10d5bfe9c4af3696d5f5; object_count=1; foreign_key_check=0; rerun_digest=stable`

The bundle creates source, backup, restore and rerun SQLite files under
`tempfile`; it does not remove legacy planning tables or authorize cleanup.

