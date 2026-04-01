# Task Spec: p2p-lan-sync

## Metadata
- Task ID: p2p-lan-sync
- Created: 2026-03-31T17:53:01+00:00
- Repo root: /Users/kirill/Documents/projects/kosmos
- Scope: Фаза 1 only (минимальный работающий sync)

## Guidance sources
- `apps/delphi/CLAUDE.md` — Delphi architecture, Ark Space, data models
- `apps/delphi/kotlin/CLAUDE.md` — Android Kotlin conventions
- `apps/eden/CLAUDE.md` — Eden architecture
- `apps/delphi/ts/src/services/sync/` — existing sync code (to be replaced)
- `apps/delphi/kotlin/app/src/main/java/com/kazui/delphi/data/sync/` — existing Android sync

## Original task statement

Replace broken P2P sync in Delphi with a reliable Syncthing-style LAN sync protocol. Hub model: Electron = WS server, Android = WS client. Diff-based sync with version vectors, batch transfer with ACK, live mode for realtime changes.

Current problems:
1. mDNS discovery doesn't work (routers block multicast)
2. Android is client-only, no bidirectional connection
3. Sync protocol is fire-and-forget with no diff, ACK, or retry

## Acceptance criteria

- **AC1**: Electron (Delphi TS) starts a WebSocket server on port `21531` at app launch when a Space is active
- **AC2**: Android (Delphi Kotlin) can connect to Electron by entering IP address in Settings (port `21531` default)
- **AC3**: After WS connection, both peers exchange `hello` messages with `device_id`, `device_name`, `protocol_version`
- **AC4**: Both peers exchange version vectors (`entity_id → hlc` maps) and compute diff locally
- **AC5**: Missing/outdated entities are sent in batches (max 100 per batch) with `sync_changes` messages
- **AC6**: Each batch is acknowledged with `sync_ack`; unacknowledged batches are retried up to 3 times
- **AC7**: After initial sync completes, both peers enter live mode — local mutations broadcast as `live_change` with ACK
- **AC8**: Connection status shown in UI: Electron (dot indicator), Android (ConnectionIndicator)
- **AC9**: Tasks created/edited/deleted on one device appear on the other within 2 seconds in live mode
- **AC10**: HLC-based LWW conflict resolution — concurrent edits resolve deterministically on both sides
- **AC11**: Existing Delphi data models (TodoItem, Project, Area, Tag, Heading, ChecklistItem) sync correctly
- **AC12**: Unit tests for: HLC comparison, version vector diff, batch splitting, message serialization
- **AC13**: `bun run dev` (Electron) and `./gradlew assembleDebug` (Android) build without errors

## Constraints

- No VPS, no relay server, no internet required — LAN only
- Hub model: Electron = server, Android = client (no WS server on Android)
- WebSocket transport (not QUIC)
- JSON messages over WS
- Must work with existing Ark Space code (space code = mesh secret for auth)
- Must preserve existing Delphi data models and Pinia/Room stores
- Port `21531` (configurable later, hardcoded for Phase 1)
- Batch size: max 100 entities or 1MB
- ACK timeout: 10 seconds for batch, 5 seconds for live change
- Max 3 retries per batch/change
- HLC format: `ISO_TIMESTAMP-COUNTER-DEVICE_ID`

## Non-goals

- mDNS auto-discovery (Phase 3)
- QR code pairing (Phase 3)
- Version vector hash optimization (Phase 4)
- Delta compression (Phase 4)
- WebSocket compression (Phase 4)
- Eden (notes) sync — Delphi tasks only for now
- Multi-hub (multiple Electron servers)
- Encryption / wss://
- Ark WS relay integration
- NDJSON append-only log (existing SQLite is sufficient)

## Verification plan

- **Build**: `cd apps/delphi/ts && bun run dev` starts without errors, WS server listening on 21531; `cd apps/delphi/kotlin && ./gradlew assembleDebug` succeeds
- **Unit tests**: `cd apps/delphi/ts && bun run test` — sync engine tests pass; Kotlin unit tests for sync engine
- **Integration tests**: Manual — Android connects to Electron, tasks sync bidirectionally
- **Lint**: `cd apps/delphi/ts && bun run lint` passes
- **Manual checks**: Create task on Android → appears on Electron; create on Electron → appears on Android; edit on one → updates on other; delete on one → deleted on other
