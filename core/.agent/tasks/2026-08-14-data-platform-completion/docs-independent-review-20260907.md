# Independent Docs review — 2026-09-08

Reviewed exact Core PR #73 head `ede4e8c352959bff16e7f13be92481605151886b` against frozen spec
`056c71d585a1af4e3cc7c47eb05dae98f61ef5c8`.

| Criterion | Verdict |
| --- | --- |
| AC1 | `PASS_WINDOWS_ONLY` |
| AC2 | `PASS_WINDOWS_ONLY` for the tested Core scope |
| AC3–AC7 | `PARTIAL`; the concrete missing artifacts remain listed in `ac2-9-gap-matrix.md` |
| AC8 | `PASS_WINDOWS_ONLY`; focused local-state regression passes 1/1 |
| AC9 | `PASS_WINDOWS_ONLY`; profile persistence, typed tombstone path and actual loopback transport matrix pass |
| AC10 | `NOT_RUN` / `OPEN`; retained legacy planning and recovery consumers still violate the literal removal requirement |
| AC16 | `PARTIAL`; independent final-candidate review passes, final merge and post-merge issue evidence are pending |

Final-candidate review: PASS for the AC9 claims. The diff includes the actual
`SyncServer::new` + `SyncClient::new` loopback, full/metadata/none projection,
definitions-before-objects, required link, usage/secrets/local-state
exclusion, reconnect/replay, narrowing, and delete→typed tombstone→filter→
apply. Evidence pins `data_platform_phase7` 6/6, typed tombstone 1/1 and the
transport matrix 1/1. This verdict is Windows-only and does not claim
cross-platform or GUI coverage.

Provenance review: PASS. The stale release-metadata reference is removed and
the Core RPC fixture source uses the full SHA
`0b06342014238244749946bbe3db92d94dfa0868`.

The older obsolete-resolver guard result is not treated as a current PASS;
current Cortex merge `c630b4c` is recorded separately from that pre-merge
guard run. Delivery is **not complete**. Windows-only scope is respected;
Linux/macOS and visible GUI remain deferred by user. No cleanup or legacy
removal is authorized.
