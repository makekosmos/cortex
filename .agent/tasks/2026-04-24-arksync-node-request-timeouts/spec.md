# @arksync/node Request Timeouts

## Context

Self-managed `@arksync/node` sidecar requests now carry request ids, but a request can still remain pending forever if `ark-core-rpc` stalls after accepting stdin. That makes lifecycle and CRUD calls hard to reason about in Electron main services.

## Scope

Add a bounded request timeout for self-managed sidecar requests.

## Acceptance Criteria

- AC1: `ArkClientOptions` exposes an optional `requestTimeoutMs`.
- AC2: Self-managed sidecar requests use a default timeout when `requestTimeoutMs` is not provided.
- AC3: When a request times out, the pending request is removed and its promise rejects with a useful error containing the operation name.
- AC4: When a response is matched before the timeout, the timer is cleared and the promise resolves/rejects only from that response.
- AC5: `failAll` and direct write failure clear request timers before rejecting or removing pending requests.
- AC6: Injected `requestFn` mode is unchanged and is not wrapped in SDK-side timeouts.
- AC7: Fresh TypeScript verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Cancellation messages to `ark-core-rpc`.
- Killing/restarting the sidecar on timeout.
- Changing Rust sidecar behavior.
