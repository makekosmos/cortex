---
name: offline-first
description: Offline-first development patterns for Kosmos apps. Use when adding CRUD, sync, or data layer features to ensure the app works without network.
---

# Offline-First Skill

## Core Principle

**Local state is the source of truth. Network is a side effect.**

The user must never wait for a network response to see the result of their action.

## The Three Laws

1. **Write local first** — every mutation hits local storage before touching the network
2. **Queue, don't fail** — if the network is unavailable, enqueue the change, never show an error
3. **Merge on reconnect** — when the connection returns, flush the queue and reconcile state

## Pattern: Optimistic Updates

```typescript
// Vue/Pinia — Web
function addTodo(params: CreateTodoParams): TodoItem {
  const todo = createTodoItem(params);
  todos.value = [todo, ...todos.value]; // 1. Update UI immediately
  arkSync.sendChange(todoItemToArkChange(todo, "create")); // 2. Queue/send async
  return todo;
}

function updateTodo(id: string, patch: Partial<TodoItem>) {
  todos.value = todos.value.map((t) => (t.id === id ? { ...t, ...patch } : t)); // 1. Optimistic
  const updated = todos.value.find((t) => t.id === id);
  if (updated) arkSync.sendChange(todoItemToArkChange(updated, "update")); // 2. Async
}
```

```kotlin
// Android — Kotlin/Compose
fun completeTodo(id: String) {
    viewModelScope.launch {
        val updated = todoDao.getById(id)?.copy(
            isCompleted = true,
            completedAt = Instant.now().toString()
        ) ?: return@launch
        todoDao.upsert(updated)                                    // 1. Write to Room
        syncClient.sendChange(ArkEventMapper.todoToArkChange(updated, "update"))  // 2. Queue/send
    }
}
```

**Rule**: Never `await` the sync call in a UI handler. The UI should never block on network.

## Pattern: Durable Outbox Queue

Changes made while offline must survive app restarts.

### Web (localStorage)

```typescript
const OUTBOX_KEY = "delphi.sync_outbox";

function enqueue(change: ArkChange) {
  const outbox = loadOutbox();
  outbox.push(change);
  localStorage.setItem(OUTBOX_KEY, JSON.stringify(outbox));
}

function flushOutbox() {
  const pending = loadOutbox();
  localStorage.setItem(OUTBOX_KEY, "[]"); // Clear before sending
  for (const change of pending) sendChange(change);
}
```

### Android (Room)

```kotlin
@Entity(tableName = "pending_changes")
data class PendingChange(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val payload: String,   // Serialized ArkChange JSON
    val createdAt: String,
)

// Queue:
pendingChangeDao.insert(PendingChange(payload = json.encodeToString(change), ...))

// Flush on reconnect:
pendingChangeDao.getAll().forEach { item ->
    session.send(item.payload)
    pendingChangeDao.deleteById(item.id)  // Delete AFTER confirmed send
}
```

**Rule**: Delete from outbox only after the send is confirmed. On crash/restart, unsent items are retried.

## Pattern: Version Vectors

Every device tracks what it has seen from every other device.

```json
{ "delphi-android-abc": 87, "delphi-web-xyz": 42 }
```

On reconnect:

1. Send vector to server: `sync_start { vector }`
2. Server returns everything the client missed: `sync_changes { changes }`
3. Client applies missed changes, then flushes outbox

**Rule**: Vector only increases. Never reset or downgrade a device entry.

**Rule**: Persist vector to durable storage (DataStore, localStorage, SQLite) — never keep only in memory.

## Pattern: Idempotent Writes

Incoming changes from other devices must be safe to apply multiple times.

```typescript
// Web — Pinia upsert
function upsertTodo(incoming: TodoItem) {
  const idx = todos.value.findIndex((t) => t.id === incoming.id);
  if (idx >= 0)
    todos.value[idx] = incoming; // Update
  else todos.value.push(incoming); // Insert
}
```

```kotlin
// Android — Room @Upsert
@Upsert
suspend fun upsert(todo: TodoItem)  // INSERT OR REPLACE semantics
```

**Rule**: Use `source_id` (the entity UUID) as the deduplication key, not `event_id`.

## Pattern: Soft Deletes

Never physically remove data. It must be recoverable and syncable.

```kotlin
// Wrong:
todoDao.deleteById(id)

// Right:
todoDao.upsert(todo.copy(isTrashed = true))
syncClient.sendChange(ArkEventMapper.todoToArkChange(updated, "delete"))
```

For sync protocol, `change_type: "delete"` tells peers to mark the item deleted locally — not to issue a `DELETE` SQL.

## Pattern: Connection State UI

The UI should reflect sync state without blocking interaction.

```typescript
type ConnectionState = "online" | "syncing" | "offline";

// Green dot → online
// Yellow pulsing → syncing/connecting
// Red → offline
```

```kotlin
enum class SyncStatus { OFFLINE, SYNCING, ONLINE }
val status: StateFlow<SyncStatus> = syncClient.status
```

**Rule**: Show status indicator but never disable CRUD operations based on connection state.

## Pattern: Batch Incoming Changes

When processing a burst of changes from the server, batch DB writes for performance.

```kotlin
val todosToUpsert = mutableListOf<TodoItem>()
changes.forEach { change ->
    ArkEventMapper.arkChangeToTodoItem(change)?.let { todosToUpsert.add(it) }
}
if (todosToUpsert.isNotEmpty()) {
    try {
        todoDao.upsertAll(todosToUpsert)  // Single transaction
    } catch (e: Exception) {
        // FK violation — fall back to per-item with skip
        todosToUpsert.forEach { todo ->
            try { todoDao.upsert(todo) } catch (ex: Exception) { /* skip */ }
        }
    }
}
```

## Pattern: Reconnect with Exponential Backoff

```kotlin
var delay = 1_000L
while (active) {
    try {
        connect()
        delay = 1_000L  // Reset on success
    } catch (e: Exception) {
        delay = minOf(delay * 2, 30_000L)  // Cap at 30s
        kotlinx.coroutines.delay(delay)
    }
}
```

## Checklist: New Feature

Before marking any data feature done:

- [ ] Does the operation succeed without network? (no spinner, no error)
- [ ] Is the local state updated synchronously before the sync call?
- [ ] Is the outbox change persisted to durable storage (not just memory)?
- [ ] Is the incoming-change handler idempotent (safe to apply twice)?
- [ ] Are deletes soft (flag or event), not physical?
- [ ] Does the UI work in airplane mode with no sync service running?
- [ ] Is the connection state indicator correct (but not blocking UI)?

## Anti-Patterns to Avoid

| Anti-Pattern                          | Why Bad                              | Fix                                       |
| ------------------------------------- | ------------------------------------ | ----------------------------------------- |
| `await syncClient.send(change)` in UI | Blocks on network; shows spinner     | Fire-and-forget, queue if offline         |
| Show "offline" error and block action | Hostile UX, violates offline-first   | Queue silently, sync later                |
| Outbox in memory only                 | Lost on app restart                  | Persist to Room / localStorage            |
| Physical `DELETE` for user deletions  | Can't sync, unrecoverable            | Use `isTrashed` flag or soft-delete event |
| Version vector in memory only         | App restart → re-fetches all history | Persist to DataStore / localStorage       |
| Apply changes then update vector      | Race condition: re-apply on restart  | Update vector inside apply loop           |
| Fetch from server in CRUD handler     | Network dependency in hot path       | Read from local DB only                   |
