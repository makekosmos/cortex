# Evidence - Eden ark client init handshake

## Scope
- `apps/eden/ts/main/ark.ts`
- `.agent/tasks/2026-04-20-eden-ark-client-init-handshake/spec.md`
- `.agent/tasks/2026-04-20-eden-ark-client-init-handshake/evidence.json`
- `.agent/tasks/2026-04-20-eden-ark-client-init-handshake/raw/build.txt`
- `.agent/tasks/2026-04-20-eden-ark-client-init-handshake/raw/tsc.txt`

## Root cause
- `ArkClient.ensureChild()` wrote the internal `init` request and immediately flipped `initialized = true`.
- `dispatchNext()` then sent the first real request right away.
- When stdout returned the `init` response first, `flushStdout()` treated it as the response for the active user request.
- That made the first request receive `true` instead of its actual payload. In the reported crash, `list_objects` therefore produced a boolean, and `listArkObjects()` crashed on `objects.map(...)`.

## Fix
- Keep `initialized = false` until the first non-event response confirms the `init` handshake.
- In `flushStdout()`, detect event-shaped lines and skip them.
- Route the first non-event response to `settleInit()` while the client is still uninitialized.
- Only after successful init does `dispatchNext()` dequeue and write the first real request.

## Verification
- `node node_modules/typescript/bin/tsc -p tsconfig.json --noEmit`
- `bun run build`

## Acceptance mapping
- AC1: PASS - `initialized` is no longer set during `ensureChild()`.
- AC2: PASS - `flushStdout()` consumes the first non-event line as init while `initialized === false`.
- AC3: PASS - event-shaped stdout lines are ignored via `isArkEventMessage()`.
- AC4: PASS - typecheck and build passed on current code.
