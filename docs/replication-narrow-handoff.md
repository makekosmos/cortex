# Narrow integration replication handoff

This work targets Core issue #53 and the type-registration dependency of #33.
It does not close either issue or authorize the source moves in #36.

## Pinned inputs

- Target baseline: `f8c4f4be3d9c5c86ddef2221c69581e84dd6578a` (`origin/main`).
- Reference: PR #55, commit `9fe45d35c51781b6f5af99dae4c4d8a01f82b610`
  (security-fixed replication implementation).
- Historical Data Platform PR #35 is not an integration source for this change.

## Ownership and sequence

1. Core owns integration contracts, durable authorization/grants, addressed opaque
   envelopes, refresh fencing, signed-frame validation and transactional apply.
2. Core transport work follows that foundation: authenticated addressed routing,
   peer identity binding, revocation and reconnect checks. A broadcast-only
   transport must reject signed integration sends.
3. Cortex owns node keys, pairing, encryption/decryption, provider credentials,
   opaque worker handles and provider collection/refresh. Core never receives
   private keys. Cortex must use a pinned Core revision implementing the required
   operations; the presence of this document does not make an operation available.
4. An independent reviewer verifies the combined revision before release.

Core #36 source moves remain deferred until the current runtime and consumer
contracts are stable. A documentation change or a green unit suite does not
remove that dependency.

## Required security invariants

The delivered foundation exposes Rust library functions, including
`persist_node_authorization`, `persist_integration_grant`, `prepare_signed_sync`,
`validate_outbound_signed_sync` and the integration-specific `db` operations.
The contract pin requested for consumers is Core commit
`86ce336dde64589cd91019692ae8a9186ee33db4`. The current implementation,
including Iroh addressed routing, is available at
`a440f7e392d33d38393810a82ffbe73f01614afe` on
`origin/codex/core-replication-narrow`.

It exposes sidecar RPC variants and FFI exports for the four operations above,
plus the addressed `SignedIntegrationFrame`/`SignedIntegrationAck` protocol
messages and LAN/WSS send methods. Cortex can consume the validated operations
through those pinned entry points. Iroh addressed transport is available on the
current implementation; the relay broadcast transport rejects addressed
integration messages.

- Incoming frames cannot bootstrap their own node authorization or grants.
- Verify the space, signed origin, addressed recipient, signature and current
  key epoch against locally established trust before applying payloads.
- Failed apply rolls back data, vectors and replay reservation together.
- Grant/key rotation and revocation invalidate stale envelopes and queued sends.
- Publication requires the current authorized holder, generation, fencing token
  and unexpired lease, checked in the same transaction as the write.
- Envelope key IDs bind to the exact recipient encryption public key.
- Generic object synchronization must not export integration envelopes.

## FatSecret consumer contract

Register `integrations/fatsecret/type-registration.json` through the existing
`types.registerPackageDefinitions` operation, then use `upsert_object` for mapped
`nutrition_entry_obj` records. Preserve the literal persisted type identifier.
The schema artifact alone does not install a Cortex provider adapter.

On the pinned baseline, `prepare_object` validates canonical registrations and
retains a compatibility path for package registrations. The registration test
proves registry compatibility and idempotency, not package ingress enforcement.
The canonical validator itself requires a built-in canonical content contract,
so it cannot be used unchanged to validate this package registration end to end.
The existing mapper can omit an unavailable `quantity`, whereas this schema
requires an explicit nullable `quantity`. The Cortex adapter must normalize that
field and validate its mapped payload before writing it. These consumer and
ingress gaps remain part of #33; importing unrelated generic ingress changes
from PR #55 is not implied by this slice.

## Acceptance remaining for the full feature

- Authenticated two-node transport and reconnect/revocation tests.
- Collection and refresh on an authorized recipient after the original node stops.
- Black-box evidence that credentials do not reach renderer, worker or log output.
- Real provider OAuth and provider-specific concurrent-refresh limitations.
- Windows acceptance against the pinned release candidate.

None of these is implied by contract/persistence tests. Final local evidence for
the delivered slice is recorded separately from full-feature acceptance.

## Next bounded Core assignment

Expose the existing validated Rust operations through the sidecar request/handler
and FFI surfaces. Keep one writer for shared request enums and manifests. Verify
serialization and negative authorization cases through those actual entry points.
Do not report successful network transmission while only preparing signing bytes.

The current candidate completes authenticated addressed routing through Iroh
and revalidates outbound grants before sending. Network negative coverage and
independent review remain required before this revision is accepted.
Preserve the transport identity binding on receipt and revalidate grants
immediately before send. The existing broadcast-only relay implementation
must fail closed. An in-process two-database apply test is insufficient.

Cortex can work on its trusted-host adapter against the resulting pinned API
contract in a separate checkout. Shared source moves remain a later assignment.

## Local verification

Checked on Windows on 2026-09-05 against Core commit
`86ce336dde64589cd91019692ae8a9186ee33db4`:

| Check | Result |
| --- | --- |
| `rtk cargo nextest run --workspace --all-features` | PASS: 413 tests, 32 binaries (before the final 7-line relay guard) |
| `rtk cargo test --manifest-path crates/ark-core/Cargo.toml --lib` | PASS: 191 tests on this SHA |
| `rtk cargo test --manifest-path crates/ark-core/Cargo.toml --bin ark-core-rpc` | PASS: 34 tests on this SHA |
| `rtk node --test integrations/fatsecret/test/*.test.mjs` | PASS: 15 tests |
| `rtk cargo fmt --check` | PASS |
| Clippy: exact existing pre-push command in `lefthook.yml` | PASS with existing baseline |
| `rtk cargo shear` | PASS; existing unlinked-file warnings remain |
| `rtk cargo deny check advisories bans sources` | PASS; duplicate-version warnings remain; licenses not checked |
| `rtk node scripts/check-source-size.mjs` | PASS with existing grandfathered files |
| `rtk node scripts/check-ark-write-boundaries.mjs` | PASS |
| `rtk git diff --check 8b3b67e4b9622d8f2730b2900804d4ab89212ade..HEAD` | PASS |
| `rtk node scripts/ark-smoke.mjs` | FAIL at SDK step: expected sibling `../arca-sdk` absent in isolated worktree; guard and Rust tests passed |

The final relay guard was checked by the repository clippy gate and focused
tests above. Independent review of the final Iroh/network diff remains required;
these checks do not prove Iroh routing or source-offline provider collection.
