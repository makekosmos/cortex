# Task Spec - Eden ark client init handshake

## Goal

Fix the Eden main-process Ark sidecar client so the first real request does not accidentally consume the sidecar `init` response.

## Scope

- `apps/eden/ts/main/ark.ts`

## Acceptance Criteria

- AC1: The Ark client does not mark itself initialized before the `init` response is actually received.
- AC2: The first non-event response line after spawn is consumed as the `init` handshake, not as the payload for the first queued request.
- AC3: Event lines from `ark-core-rpc` are ignored by the request-response queue instead of corrupting pending requests.
- AC4: Eden build/typecheck still pass after the handshake fix.
