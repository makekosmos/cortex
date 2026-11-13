# KOS-424 application port boundary

## Grounding

The direct sync flow asks `TaskMergeDeliveryPort` for a bounded outbound batch, sends the batch through the authenticated session, and acknowledges it after the peer responds. On inbound sync, the receiver applies the batch through a DB command. SQLite stores journal entries, progress, and replay receipts in the same transaction. `sync_storage.rs` currently owns both those port contracts and unrelated entity/feed ports.

## Candidates

Candidate A moves the task-merge port and its policy identity into `ark-application`. `ark-core` depends on `ark-application`, and `ark-application` depends on `ark-domain`. Its dependency tree has no SQLite, transport, or runtime. The current `ark_core::application::sync_storage` module re-exports the moved types until callers migrate.

Candidate B first moves serializable entity, usage, object, and sync DTOs from `types.rs` into a separate `ark-types` crate or expands `ark-domain` to own them. That would create a useful lower-layer dependency for later extraction, but it does not establish the application port boundary and requires more public type migration now.

Choose Candidate A for this unit. It moves an existing cohesive port with its receiver-owned policy identity and gives the sync and SQLite implementations one shared contract. Candidate B remains a separate domain-model decision before moving the broader application modules.

## Target shape

```text
ark-core -> ark-application -> ark-domain
```

`ark-application::sync_policy` owns `PolicyIdentity`. `ark-application::task_merge` owns journal page and batch records plus `TaskMergeDeliveryPort`. `ark-core::application::sync_storage` keeps feed and entity ports. Its re-exports preserve existing Engine-facing Rust paths while the internal callers move in later KOS-424 units. Protocol serialization and SQLite persistence stay in `ark-core`.

## Behavior contract

The receiver's policy identity binds delivery to the revision and digest that selected the data. Existing task-merge wire frames, field names, optional conflict digest, batch order, idempotent ACK behavior, and policy checks remain unchanged. The real behavior pin is `sync_client_start::task_merge_tests::authenticated_direct_sync_delivers_task_merge_batches_over_the_wire`, backed by the SQLite journal and inbound batch tests.

## Decision

Move only the application-owned port and shared policy identity. Keep SQLite commands and protocol messages in `ark-core`. Keep the old module path as the current public facade, not as a second type definition. Remove that facade only after callers migrate and the public API review accepts the path change.
