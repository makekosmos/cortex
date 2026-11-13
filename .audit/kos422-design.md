# KOS-422 synthesized design

## Decision

The receiving device owns its selection. It persists a revisioned policy for its local device identity and presents that policy to authenticated senders. A sender may cache the receiver's accepted policy for enforcement, but that cache is not the authority. Sync selection remains separate from application permissions.

Use the existing `sync_profiles` and `sync_profile_rules` tables unless a concrete missing invariant is demonstrated. Replace the backend-wide `Arc<Mutex<Option<SelectiveSyncProfile>>>` with a peer-scoped storage operation used by snapshot and live-feed projection. A page operation must bind the selected policy generation to the database snapshot. Persist scan progress separately from successful delivery/ACK progress, and compare-and-swap progress against that generation so a late old-generation ACK cannot advance the current cursor.

## Safe local eviction

Narrowing a policy schedules eviction on the receiving device. In one local-only transaction, recheck the active policy generation and eligibility, record the entity's local residency transition, and remove only safely disposable materialized rows. This path must not call ordinary object deletion, advance HLC, write a tombstone, or append a global committed-feed deletion. Retain rows with pending local edits, local origin, unresolved operations, ambiguous provenance, or dependencies still needed by retained data. Return deferred item counts so the caller can explain why some space is still in use.

Add durable local residency/provenance evidence rather than infer cache cleanliness from the latest HLC, one peer cursor, or an authenticated receipt alone. All local and authenticated remote object-write paths must update that evidence in the same SQLite transaction. Dirty state clears only after a defined durable synchronization condition; otherwise retain the object. Preserve required type/link dependencies and fail closed for legacy rows without enough evidence.

Eviction requires a durable local marker so an intentionally absent row is distinct from unknown absence. On widening or reconnect, a matching already-accepted receipt may rehydrate only when that marker exists, the current generation permits the representation, and no local edit conflicts. Keep digest-mismatch protection for ordinary duplicate receipts. Metadata-to-full is a distinct representation transition and must be able to fetch content at the same source revision without weakening integrity checks. Reject stale queued or in-flight pages after narrowing so they cannot repopulate excluded data.

## Projection and scheduling

Metadata-only selection omits content fields and must not invoke content/blob loaders. Keep schema-required identity and metadata, and distinguish metadata/full representation in local residency and receipt handling. Blob selectors describe future metadata/content choices; this issue does not add a blob service.

Keep `SyncLive` responsive while `Backfill` imports older eligible data at lower priority. Do not let the new-device snapshot share the live-edit latency budget. The bounded DB command API should carry a stable principal (`device_id` for sync, app/package for RPC, system for maintenance) and command class (`Interactive`, `SyncLive`, `Backfill`, `Maintenance`) so later app-scope work does not need a second command-context migration.

## Required evidence

Production SQLite tests must prove: independent policy and restart behavior for six device identities; a real two/three-peer widen/narrow/reconnect flow after the sender cursor passed excluded rows; immediate local-only eviction with no tombstone/feed delete; retention of pending and ambiguous local state; race safety between a local edit and eviction; idempotent retry across a crash between policy update and eviction; same-revision rehydration after inbox/entity receipts already exist; stale pages/ACKs cannot overwrite or repopulate a newer policy; metadata-only has no content fields and does not call content loaders; and app grants remain unchanged.

## Review finding

Both candidate designs initially deferred eviction and therefore failed KOS-422's storage goal. The cross-judge selected receiver-owned policy plus generation-checked progress, then added guarded local eviction, durable residency/dirty evidence, and receipt-aware rehydration as release gates. See `/tmp/arena-kos422/judgment.md` for the scored review. Current application behavior returns early for a duplicate entity receipt, so resetting the sender cursor alone cannot restore a locally evicted row.

## Fresh design synthesis at f738ad3a

Two additional designs were compared at `/tmp/arena-kos422/candidate-a/design.md` and `/tmp/arena-kos422/candidate-b/design.md`, then independently cross-judged at `/tmp/arena-kos422/judgment-fresh.md`. Candidate B is the base because it makes the local receiver the policy authority, stores a separate authenticated per-peer enforcement copy, and describes both outbound projection and inbound stale-page rejection. Graft candidate A's single `outbound_page(peer, accepted, source, limit)` operation and its explicit partial-completion checkpoint. The receiving device's profile must control its inbound storage selection; the sender's accepted copy controls only what is sent to that receiver.

Before wire implementation, specify the complete correlation contract. Snapshot pages, terminal snapshot completion, live changes, receipt ACKs, and applied ACKs must all carry the accepted policy revision and digest. Validate stale pages before inbox/application mutation and stale acknowledgements before progress mutation. Existing live ACKs carry only `change_id`, and snapshot messages have no policy revision. This missing wire detail blocks integration, but not the independent domain/filter work.

Keep scan progress, delivery receipt, and applied progress distinct. Each must be bound to the accepted policy revision/digest. A policy revision is the monotonic state transition; the digest verifies canonical rules and filters. Do not infer a new policy generation from an unversioned process-global profile.

The design sequence starts with typed canonical filters and a batch projector, then durable local/accepted peer policy, authenticated exchange, unified revision-bound projection and ACKs, local residency/eviction, receipt-aware rehydration, and closeout. The mid-point checkpoint after unified projection does not close KOS-422; safe eviction and same-revision rehydration remain release gates.

The first implemented domain slice fixes `FilterV1` fields as exact, case-sensitive top-level keys in `SyncEntity.data`. A type rule's filter selects object records; the corresponding `object_type` definition is retained by selector mode and does not itself have to match object-field predicates. Link dependency closure includes only target objects that independently pass their type selector, mode and filter. This shape keeps filters independent of SQLite/wire details and avoids implicitly allowing a selected link to bypass its target's policy. The policy remains process-global and test-configured at this slice; production per-device ownership and authenticated contract exchange are still open.
