# ARK Relay Fail-Fast Contract

## Context

`ark-core-rpc` and `@arksync/node` expose relay configuration fields (`relay_url` / `relay_api_key`, `relayUrl` / `relayApiKey`), but the current sync start path does not use them. This creates a false support contract: callers can configure relay and receive a successful `start_sync` even though relay transport is not active.

## Scope

Replace ignored relay configuration with an explicit fail-fast error until relay transport is wired into the runtime.

## Acceptance Criteria

- AC1: `ark-core-rpc` `start_sync` returns an error if `relay_url` or `relay_api_key` is provided.
- AC2: `@arksync/node` `ArkClient.start()` throws a clear `@arksync/node` error if `relayUrl` or `relayApiKey` is provided.
- AC3: Starting sync without relay options remains unchanged.
- AC4: Existing injected request function mode remains unchanged except for the same local option validation when relay options are present.
- AC5: Fresh Rust and TypeScript verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Implementing relay transport.
- Changing relay server package code.
- Removing relay types from public TypeScript interfaces.
- Changing LAN/P2P sync behavior without relay options.
