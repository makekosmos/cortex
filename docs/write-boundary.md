# Write boundary

The Engine process owns the user's SQLite database. `ArkService` preserves the
request and response contract and composes the application, sync, transport,
and storage components. `ark-core` owns request admission, fairness, and the
database actor lifecycle. `ark-store` owns SQLite connections, schema and SQL
adapters, the single writer, transaction boundaries, and snapshot/restore.

## Allowed write paths

1. **Engine to `ArkHost` to `ArkService`**: in-process calls through the stable
   Engine operation surface. The service routes writes through the Core actor
   to Store-owned operations.
2. **Other processes to the Engine API**: HTTP and WebSocket clients submit
   operations; they never open the user's database.
3. **Core actor to Store**: Core authenticates and schedules a bounded command;
   Store performs the synchronous database operation on the writer connection.
   A single transaction contains each cross-row mutation and commits only
   after the operation succeeds. Network awaits never run inside a transaction.

## Bans

- No process other than the Engine opens the user's database. No sidecar,
  renderer, or package worker opens it directly.
- Application, domain, sync, and network code do not issue SQLite statements or
  retain SQLite connections. SQL belongs to `ark-store`; transport code uses
  typed sync effects and application capabilities.
- A write to a syncable entity updates the revision, sequence, tombstone,
  receipt, and committed-feed state required by that operation in the same
  transaction as the entity mutation. Direct SQL that bypasses the Store
  operation can make changes invisible to sync.
- No destructive migration (`DROP TABLE` or incompatible column rewrite) is
  allowed without a separately reviewed migration and recovery plan.
- No `Mutex::lock().unwrap()` in production paths. Recover poison with
  `unwrap_or_else(|error| error.into_inner())` or return an error when the
  invariant requires it.

## Durability and recovery

`ark-store::open_db` configures every SQLite connection for WAL,
`synchronous=FULL`, foreign keys, and a 5-second busy timeout. Successful
application acknowledgements follow the operation's SQLite commit. This is the
configured SQLite durability contract; it is not a claim that every operating
system, filesystem, storage device, or controller honors flush requests during
a sudden power loss.

Backups use SQLite's online backup API, are integrity-checked, and are
published with a manifest only after verification. Restore is a serialized
maintenance operation: it stages and validates the source, preserves device
identity, rejects a snapshot older than the live committed-feed watermark,
records recovery intent, and verifies the restored database before returning
success. A failed apply attempts rollback from a recovery capsule. If the
restore or rollback state cannot be proven, the database actor fails closed and
startup recovery must finish before the service accepts requests.

On Unix, backup publication syncs the containing directory. On Windows, the
portable filesystem path syncs and verifies the files but does not flush
directory metadata, so sudden power loss can lose newly published backup
names. SQLite database commit durability is separate from backup-name
durability. The test suite injects SQLite disk-full, commit, panic, and restore
failures and exercises stale-restore refusal with an existing peer; it does not
simulate physical power loss.

## Verifying a new write path

1. The write reaches the Core actor through an authenticated, bounded command.
2. Store owns the SQLite operation and transaction; no connection escapes to
   the caller.
3. If the entity syncs, its sync state and committed-feed entry are updated in
   the same transaction as the entity mutation.
4. Errors cross the Engine boundary as typed service errors and the existing
   JSON response shape; failures do not return success or empty data.
5. Tests use Store fixtures, temporary directories, or explicit test-support
   capabilities and never a real user data directory.
