# ARK Sidecar Request IDs

## Context

`ark-core-rpc` currently accepts JSON-line requests without request ids and returns response lines without ids. Clients must rely on strict one-request-at-a-time queueing to match responses to callers. Async event frames share stdout with responses, so the protocol needs a stable response correlation field before the SDK can safely evolve toward concurrent requests.

## Scope

Add backward-compatible request id support to the Rust sidecar protocol and make the self-managed `@arksync/node` child-process client send and validate ids.

## Acceptance Criteria

- AC1: `ark-core-rpc` accepts legacy requests without `id` and still emits legacy `{ ok, data/error }` responses.
- AC2: `ark-core-rpc` accepts requests with an `id` field and echoes that same `id` on success and error responses.
- AC3: Async event frames remain unchanged and do not include request ids.
- AC4: The self-managed `@arksync/node` child-process path attaches unique request ids and resolves/rejects responses by matching response ids.
- AC5: Injected `requestFn` mode remains backward-compatible and does not force ids into Delphi's existing sidecar client path.
- AC6: Fresh Rust and TypeScript verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Updating Delphi and Eden sidecar wrappers to multiplex requests.
- Changing sync WebSocket protocol messages.
- Replacing JSON-lines with another transport.
- Full timeout/cancel support.
