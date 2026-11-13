# KOS-424 shared task-merge delivery

## Problem

LAN sync and RelaySync independently read the accepted policy identity, prepare a bounded task-merge batch, recheck that identity, and persist acknowledgements. Their readiness checks and send mechanisms differ, so the shared layer must own only policy-bound delivery state and leave endpoint admission and transport I/O in `ark-core`.

## Usage

The transport adapter continues to check that a peer is admitted, ready, and negotiated for task-merge bootstrap. It then calls `ark_sync::task_merge_delivery::prepare_outbound_batch(&storage, peer_id).await` and sends the returned batch through its own transport. After a validated acknowledgement, it calls `acknowledge_outbound_batch(&storage, peer_id, &policy_identity, &batch_id, through_position).await`; it repumps only when the acknowledgement matched current policy.

## Shape

`ark-application::PeerSyncPolicyPort` owns the accepted peer policy identity lookup because committed-feed and task-merge delivery both depend on it. `TaskMergeDeliveryPort` returns a typed policy refusal from preparation, so the sync layer can suppress ineligible data without matching error text. Other storage failures remain errors.

`ark-sync::task_merge_delivery` owns the sequence that reads the identity, asks storage for at most 100 journal entries, then reads the identity again before exposing the batch. Missing or changed identity and policy refusal produce no batch. ACK coordination checks the identity before calling storage; the storage command keeps its transactional policy check as the final race guard. These operations are transport-neutral and do not pick peers, inspect capabilities, send messages, fan out, or delete data.

The two independently sketched designs agreed on this narrow boundary. A wider coordinator that owns authentication or send behavior was rejected because those facts and APIs belong to different transport adapters. Keeping the duplicated storage sequence was rejected because it leaves two authorities for policy-bound batch and ACK behavior.

## Tradeoffs accepted

- We accept one extra policy identity read after preparation in exchange for suppressing stale batches before a transport sees them.
- We accept that a policy can still change after the final read; each batch carries its identity and storage/receiver checks remain authoritative.
- We accept a no-op when no accepted identity exists, since there is no policy authorizing transfer.

## Implementation reconciliation

The identity lookup moved into a separate `PeerSyncPolicyPort` rather than `TaskMergeDeliveryPort`. Both committed-feed and task-merge delivery use the identity, so the narrower shared port avoids assigning policy state to either delivery protocol.

## Open questions and risks

There is no positive Relay task-merge delivery wire test yet. Relay's existing admission/rejection tests and the shared coordinator tests cover the changed decisions, while the real batch-over-wire test covers LAN. Add a Relay positive wire test when the relay test harness can exercise that path without broadening this extraction.

Selective sync remains transfer policy only: disabling a category on a device stops transfer there and leaves its local records intact. A device that wants books can receive them; one that does not keeps its existing local state and does not receive new copies.

## Implementation result

Policy identity lookup now uses the shared `PeerSyncPolicyPort`, and policy refusal is typed. LAN and Relay share preparation and ACK coordination while keeping admission and sends transport-specific. The remaining work for this unit is final verification and commit; no wire payload or message changed.
