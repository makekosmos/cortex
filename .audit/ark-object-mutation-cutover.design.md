# ARK object-mutation cutover

## Problem

The object upsert, delete, canonical game, and book-cover commands still execute through `DbCommand::execute(&Transaction)`. Their Core adapter mixes Core-owned canonical validation with Store persistence helpers and direct SQL. Moving only the adapter to Store would leave Application sequencing a list of persistence stages and would not remove the raw transaction from Core. The cutover must preserve one admitted immediate transaction per command while moving the transaction boundary and complete persistence operation into Store.

## Usage

Core validates the command principal and submits one typed operation to the writer actor. The Store opens one immediate transaction, reads the facts needed for the decision, calls pure Application/Domain planning code, applies the resulting operation, and returns a typed outcome. The callback stays synchronous. Core does not receive a transaction or a sequence of storage methods.

```rust
// Core actor task
fn run(self: Box<Self>, writer: &mut SqliteWriter, on_error: &mut dyn FnMut(&DbError)) {
    let result = writer.execute_object_mutation(self.context, self.command);
    settle(map_object_mutation_result(result), self.reply, on_error)
}

// Store writer API
pub fn execute_object_mutation(
    &mut self,
    command: ObjectMutationCommand,
) -> Result<ObjectMutationResult, ObjectMutationFailure>;

// Pure application planning, called by Store after it reads transaction-local facts
pub fn plan_object_mutation(
    command: ObjectMutationCommand,
    facts: ObjectMutationFacts,
) -> Result<ObjectMutationPlan, ObjectMutationError>;
```

## Shape

`ark-application` owns the command inputs, typed outcomes, and pure decisions. `ark-domain` owns canonical object validation and shared canonical values. `ark-store` owns the closed operation dispatcher, SQLite reads, the single immediate transaction, and atomic persistence. `ark-core` owns principal admission, actor fairness, cancellation, lifecycle, and Engine error mapping.

The Application plan is a closed enum with complete persistence intent. It is not a list of table writes. Store matches it exhaustively and performs all associated object, link, revision, vector, feed, and tombstone changes before commit. Transaction-local facts use domain values such as object identity, snapshot, links, and resolved type registration. No SQLite row or handle crosses the crate boundary.

The Store command also handles `core.task` deletion. It routes that case through the existing Application task-mutation rules and Store task-merge persistence inside the same transaction. Store never calls back into Core. Canonical type resolution reads the persisted registry through Store; pure validation runs in Domain. Game and book-cover request/result values move to Application or Domain before Store imports them.

The actor task keeps the existing admission class, weight, bounded queue, cancellation-before-start, settlement, and reply-after-slot-release rules. It calls the Store operation directly. It does not wrap the call in `SqliteWriter::immediate`, so Store creates exactly one transaction and cannot accidentally nest one.

## Synthesis decision

Candidate A is the base because a closed Store operation hides transaction and persistence ordering. Candidate B's separation of pure Application rules from persistence ownership is retained, but its multi-method session is not the production boundary. The cross-judge identified that Candidate A must not move policy into Store, must handle task deletion without calling Core, and must match its closed result enum at each actor task. This design makes Application planning pure and gives Store one exhaustive dispatcher.

Candidate A's proposed generic result trait is rejected because it would add a second dispatch protocol beside the closed operation. Candidate B's callback over a staged persistence session is rejected because it leaves Application responsible for ordering persistence stages and does not define integration with the current writer actor.

## Tradeoffs accepted

- Store imports Application planning types so it can execute one complete operation. Application and Domain remain independent of Store.
- The actor needs a specialized object-mutation task while other command families still use `DbCommand`. Each task uses the same admission and settlement path.
- Moving game and book-cover value types and canonical ingress rules expands this slice. Keeping those Core-owned would create a dependency cycle or a temporary adapter that would need later removal.
- The Core SQLite boundary remains open until all command families migrate. This slice does not claim the final ownership gate passes.

## Alternatives considered

The staged `ObjectMutationSession` design keeps more policy in Application, but exposes many persistence steps and makes the caller responsible for their order. A Core-owned adapter that delegates SQL to Store helpers preserves the existing transaction API but cannot remove Core's `rusqlite` dependency. The closed Store operation avoids both problems and matches the accepted transaction ownership decision.

## Implementation reconciliation

The first implementation unit moved pure canonical object-write validation and the retired task-type identifier into `ark-domain`. The second moved persisted type resolution and the canonical ingress entry point to `ark-store`, migrated callers, and deleted the Core ingress module. Object upsert/delete inputs and commit types belong to `ark-application`; local book-cover inputs and planning do as well. Core keeps actor admission and settlement and posts user-visible events after the Store result commits.

The object-mutation cutover now covers local object deletion, local object upsert, book-cover replacement, and canonical game upsert. Application owns the game RPC value types, canonical validation, link-validation decisions, no-op comparison, and a complete write plan. Store reads the current game and link targets, invokes that plan, and persists the object, local state, quarantine, links, and sync version under one `IMMEDIATE` transaction. The Core game facade re-exports the stable DTO paths and delegates reads to Store; the old Core SQL projection/upsert implementation and `ObjectMutationSession` adapter have been deleted. The writer actor preserves command admission, cancellation, settlement, and the `arrancador.changed` event contract.

The game cutover verification covers DTO wire names, planning, device selection, no-op results, typed invalid outcomes, projection, local/quarantine state, link replacement, sync-state stability, late-failure rollback, executable-version usage aggregation, and the live RPC event path. Candidate files and the earlier cross-judge verdict are `/tmp/ark-object-mutation-candidate-a.md`, `/tmp/ark-object-mutation-candidate-b.md`, and `/tmp/ark-object-mutation-judge.md`.

## Open questions and risks

- The task-delete path must preserve its current operation journal, conflict guards, revision, and tombstone behavior when Store owns the command boundary.
- Game upsert and book-cover replacement have separate transactional side effects. The game integration test injects failure after object, local, quarantine, and link writes and verifies rollback; the book-cover RPC rollback test remains its evidence.
- The actor queue serializes one writer today. The dedicated task must use the same slot-release and shutdown behavior rather than creating another queue.

## Next implementation step

Continue with the broader remaining Store/Sync/Net ownership map in the completion plan. The object-mutation command boundary no longer has a Core transaction adapter.
