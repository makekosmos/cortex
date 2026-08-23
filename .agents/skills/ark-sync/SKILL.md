---
name: ark-sync
description: Ark sync protocol patterns — HLC, version vectors, WebSocket relay, P2P mesh, HMAC auth. Use when working on sync, real-time, or multi-device features in core/ark/ or any client.
---

# Ark Sync Skill

## Core Concepts

### Event Model

Every data change is an **immutable event**. Never mutate events; append new ones.

```json
{
  "event_id": "<uuid>",
  "change_type": "create|update|delete",
  "device_id": "delphi-android-abc",
  "device_seq": 42,
  "data": {
    "event_type": "task",
    "category": "productivity",
    "source": "delphi-android",
    "source_id": "<entity-uuid>",
    "summary": "Human-readable text",
    "occurred_at": "2025-03-29T10:00:00Z",
    "data": {/* full entity fields */}
  }
}
```

**Critical invariants:**

- `source_id` = idempotency key — same source_id = same entity (server upserts by source_id ALONE, ignoring source)
- **All UUIDs MUST be lowercase** — Swift `.lowercased()`, TS `.toLowerCase()`, Kotlin `.lowercase()`. Mac UUIDs are uppercase by default; failure to normalize causes duplicate entities.
- `device_id + device_seq` = unique causal sequence per device
- `device_seq` is **strictly monotonic** per device — never goes backward
- All timestamps **ISO 8601 UTC** (suffix `Z`, never local time)
- Deletions use `change_type: "delete"` — never physical removal

### Hybrid Logical Clock (HLC)

Format: `<ISO8601_walltime>:<counter:06d>:<device_id>`
Example: `2025-03-29T14:30:00.123456Z:000042:mac-a1b2c3d4`

Purpose: deterministic cross-device ordering without trusting wall clocks.

Operations:

- `tick()` — advance counter for local event
- `merge(remote)` — incorporate remote causality before comparing

Comparison: **lexicographic** (walltime → counter → device_id).

Kotlin: `HLC.kt` | Python: `core/hlc.py` — both mirror the same algorithm.

### Version Vectors

Structure: `{ device_id → last_seq_seen }`

```json
{ "mac-abc": 142, "android-def": 87 }
```

Rules:

- **Only increases** — never downgrade a vector entry
- Persisted across restarts (DataStore on Android, localStorage on Web, SQLite on server)
- On connect: send current vector → server returns missed changes → flush outbox

## WebSocket Protocol

### Connection Flow

```
CLIENT → SERVER:  sync_start { device_id, platform, vector }
SERVER → CLIENT:  sync_changes { changes, is_full_sync, server_epoch }
CLIENT → SERVER:  change { event_id, change_type, data }
SERVER → CLIENT:  change_ack { event_id, device_seq }   ← to sender
SERVER → OTHERS:  change { ... + device_id, device_seq } ← broadcast (excludes sender)
SERVER → CLIENT:  ping (every 30s)
CLIENT → SERVER:  pong
```

### server_epoch

- UUID generated once per DB lifetime, stored in `sync_meta` table
- Included in every `sync_changes` response
- When client detects epoch change → server DB was wiped → reset vector, push all local data, do NOT delete local data

### is_full_sync

- `true` when client sent empty vector (first connect or after vector reset)
- Controls when `sendMissingToServer` and zombie cleanup run

### Critical Rules

- **Never broadcast to sender** — server must exclude origin device_id
- Update version vector **after** applying each change, not before
- Flush outbox **after** processing `sync_changes`, not before
- **`sendMissingToServer`** only on `is_full_sync=true` OR `epochChanged=true` — NEVER on every reconnect (causes duplicate spam)
- **Zombie cleanup** only when `is_full_sync=true AND epochChanged=false` — on epoch change, client is authority
- **Never auto-reset version vectors** based on task count comparison — this is dangerous

## Offline Queue (Outbox)

### Invariants

1. Persist change locally before attempting to send
2. Delete from outbox only after server acknowledgement
3. Resend outbox in original order on reconnect

### Android (Room)

```kotlin
// Queue (offline):
pendingChangeDao.insert(PendingChange(payload = json.encodeToString(change), ...))

// Flush (on reconnect):
val pending = pendingChangeDao.getAll()
pending.forEach { item ->
    val change = json.decodeFromString<ArkChange>(item.payload)
    session.send(json.encodeToString(change.copy(device_id = deviceId, device_seq = ++seq)))
    pendingChangeDao.deleteById(item.id)  // ← Delete AFTER send succeeds
}
```

### Web (localStorage)

```typescript
// Queue:
this.outbox.push(change);
saveOutbox(this.outbox);

// Flush (in onopen handler):
const pending = [...this.outbox];
this.outbox = [];
saveOutbox([]);
for (const c of pending) this.sendChange(c);
```

## Applying Incoming Changes

Always use **upsert** (idempotent). Batch writes when possible.

```kotlin
// Android — batch then fallback:
try {
    todoDao.upsertAll(todosToUpsert)  // single transaction
} catch (e: Exception) {
    todosToUpsert.forEach { todo ->
        try { todoDao.upsert(todo) }
        catch (ex: Exception) { Log.w(tag, "Skipping: ${ex.message}") }
    }
}
```

### Event Type Routing

```kotlin
when {
    ArkEventMapper.isTaskChange(change) -> {
        if (change.change_type == "delete") todoDao.deleteById(change.data.source_id)
        else ArkEventMapper.arkChangeToTodoItem(change)?.let { todoDao.upsert(it) }
    }
    // add project, area, tag handlers here
}
```

## P2P Mesh (LAN)

### mDNS Discovery

- Service type: `_ark-sync._tcp.local.`
- Python: `server/discovery.py` — `ArkServiceBroadcaster` + `ArkServiceDiscoverer`

### HMAC Peer Authentication

```python
# On connect:
mesh_id = SHA256(mesh_secret)[:16]
nonce = random_hex_64()
auth_hmac = HMAC_SHA256(mesh_secret, nonce)

# On receive — ALWAYS use constant-time comparison:
hmac.compare_digest(expected, provided)  # Never use ==
```

### Loop Prevention

Every forwarded change carries `hop_path: [device_a, relay, ...]`.
Before forwarding, check: is current device already in hop_path? If yes, drop the message.

## Conflict Detection

Conflict = same `event_id` modified by two devices while both offline.

Detection (server-side):

```python
local_unsent = query(event_id=X, device_id=self, synced=0)
if local_unsent and change_type in ("update", "delete"):
    insert into sync_conflicts(local_data, remote_data, ...)
```

Resolution strategies:

1. **Last-write-wins (auto)**: compare `updated_at` timestamps
2. **Manual**: store in `sync_conflicts`, surface to user
3. **Custom**: domain-specific merge (implement per event_type)

## Common Pitfalls

| Pitfall                                         | Fix                                                                 |
| ----------------------------------------------- | ------------------------------------------------------------------- |
| Broadcasting change back to sender              | Exclude by `device_id` in `broadcast()`                             |
| Not updating vector after apply                 | Update vector inside the apply loop, not after                      |
| Not persisting vector across restarts           | DataStore / localStorage / SQLite — always persist                  |
| Physical delete instead of soft delete          | Use `change_type: "delete"`, never `DELETE FROM`                    |
| Confusing `event_id` with `source_id`           | `event_id` = server-assigned UUID; `source_id` = client's entity ID |
| Sending outbox before processing `sync_changes` | Process missed changes first, then flush outbox                     |
| Using `==` for HMAC comparison                  | Always `hmac.compare_digest()` (timing attack prevention)           |
| Uppercase UUIDs from Swift                      | Always `.lowercased()` on send, `.toLowerCase()` on receive         |
| Server upsert by `(source, source_id)`          | Upsert by `source_id` alone — different sources = same entity       |
| `sendMissingToServer` on every reconnect        | Only on `is_full_sync` or `epochChanged` — causes duplicate spam    |
| Auto-reset vector by count comparison           | NEVER — dangerous, loses data created offline                       |
| Zombie cleanup on epoch change                  | NEVER — client is authority when server was wiped                   |

## Reconnection Backoff

```kotlin
var delay = 1_000L
while (shouldReconnect) {
    try { connect(); delay = 1_000L }
    catch (e: Exception) {
        delay = minOf(delay * 2, 30_000L)
        kotlinx.coroutines.delay(delay)
    }
}
```

## Event Type Registry

| Domain   | `event_type` | `source` prefix                    |
| -------- | ------------ | ---------------------------------- |
| TodoItem | `"task"`     | `"delphi-android"`, `"delphi-web"` |
| Project  | `"project"`  | `"delphi-web"`                     |
| Area     | `"area"`     | `"delphi-web"`                     |
| Tag      | `"tag"`      | `"delphi-web"`                     |

## Debug SQL

```sql
-- Recent changes per device
SELECT device_id, device_seq, event_id, change_type
FROM sync_outbox ORDER BY device_seq DESC LIMIT 20;

-- Unresolved conflicts
SELECT id, event_id, local_device, remote_device
FROM sync_conflicts WHERE resolved = 0;

-- Version vector state
SELECT device_id, peer_id, last_seq FROM sync_vectors;
```

## Self-Check Before Finishing

- [ ] `source + source_id` uniquely identifies entity
- [ ] `device_seq` monotonically increases — no resets
- [ ] Version vector persisted to durable storage
- [ ] Changes queued locally before send attempt
- [ ] Outbox flushed after `sync_changes`, not before
- [ ] Server excludes sender from broadcast
- [ ] Soft deletes used (flag or `change_type: "delete"`)
- [ ] HMAC comparison uses constant-time function
- [ ] All timestamps in UTC with `Z` suffix
- [ ] Hop path checked before forwarding (loop prevention)
