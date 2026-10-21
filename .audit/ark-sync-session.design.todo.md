# ARK sync-session extraction design

- [x] Ground: inspect existing Core session loops, `ark-sync`, `ark-net`, and the Stage 2 boundary rationale.
- [x] Sketch: compare two independently reasoned ownership and interface designs.
- [x] Agree: synthesize the smallest boundary that meets the Stage 2 ownership target.
- [ ] Implement: move one complete, behaviorally testable session unit at a time.
  - [x] Replace server and client paired outbound-snapshot flags with the `ark-sync` progress state; preserve separate sent and acknowledged checks.
  - [x] Move outbound snapshot page sequencing into `ark-sync`; Core handles typed page and terminal-send effects.
  - [x] Move server-Hello authentication, space, and protocol-selection validation into `ark-sync`.
  - [x] Add bounded typed `SessionIo` in `ark-sync` and a supervised WebSocket adapter in `ark-net`; poll reader and writer futures concurrently under one supervisor.
  - [x] Wire the Core client loop to typed bounded session input/output and remove the Core-owned unbounded WebSocket writer task; keep Core protocol and admission handling in place for the next extraction.
  - [x] Run bulk snapshot production outside the receive loop and give ACK/admission messages a bounded priority queue so bulk backpressure does not suspend protocol reads.
  - [x] Verify authenticated peer identity is used for registry-policy admission, including a discovered `seed-*` identity; verify backpressure and teardown with focused tests and targeted clippy.
  - [x] Replace the server's unbounded WebSocket message queues with typed bounded `SessionIo`; keep outbound admission and signed-frame authorization at the socket write boundary.
  - [x] Move direct-server Hello ordering, self-connect validation, and pending blocked-peer transition into a per-connection `ark-sync` session; Core retains the block-list read, stale-peer check, registry admission, and socket effects.
  - [x] Move the policy-bound sync-start phase into that session so it starts once, only after the peer policy has persisted; Core retains persistence and initial-vector loading.
  - [x] Move client `SessionIo` receive and frame classification into ArkSync typed session inputs; Core still selects and executes application and transport effects.
  - [x] Move the server bounded receive pump and frame classification into ArkSync typed session effects; Core still interprets and executes those effects.
  - [x] Add a bounded close-after-flush handshake to `SessionIo` and verify that a terminal WebSocket frame arrives before the peer observes socket close.
  - [x] Track each server transport supervisor separately so handler cancellation signals shutdown and server stop joins the transport task.
  - [x] Move direct-session Ping/Pong control handling into the ArkSync runner.
- [x] Fail the session closed when the bounded control queue is full instead of suspending protocol reads; capacity-one regression covers saturation.
  - [x] Move connected-session effect execution into a private ArkSync driver without exposing a generic effect algebra to Core.
- [x] Scrap: ArkSync owns protocol state and typed effects; Core owns storage, application policy, callbacks, and concrete transport effects. No Core policy or concrete transport dependency was introduced into ArkSync.

## Private connected-server driver synthesis

Two independent candidates converge on one ArkSync owner per direct server connection. The owner consumes `SessionIo`, holds protocol, admission, snapshot, and feed state, and exposes an opaque handle for application intents. Core keeps the listener, task set, peer directory, catalogue policy, and callbacks. ArkNet keeps socket framing and polls reader and writer futures under one transport supervisor. Core tracks that supervisor separately so it can join it after a connection handler is cancelled.

Use Candidate A for the safety contract and Candidate B to trim the host callbacks. The cross-judge scored A 23/25 and B 22/25, then recommended this synthesis. Keep a private one-shot `PeerActivation` so Core can compare the exact directory entry while ArkSync publishes readiness. Keep the connection lease inside ArkSync. Construct `ResponderHelloRequest` when the first Hello arrives, matching current configuration timing.

The driver must retain each owned send future and its exact frame while it selects inbound work. It must not select-cancel an in-flight storage operation. It must record a terminal snapshot ticket before exposing its frame and keep inbound and outbound progress separate. Control-queue saturation now fails the session closed; preserve this behavior unless the driver introduces a retained control-send future that continues polling input. Add a close-after-flush handshake because `SessionIo::finish` currently signals shutdown before proving queued rejection frames drained. Preserve connection `JoinHandle`s through graceful timeout, abort remaining tasks, and await every task and transport supervisor.

Candidate sketches and review: `/tmp/ark-session-driver-candidate-a.md`, `/tmp/ark-session-driver-candidate-b.md`, `/tmp/ark-session-driver-judge.md`. The private connected-server driver is implemented; Core retains application-host behavior and opaque session handles. Focused ArkSync, ArkNet, Core sync-server, and round-trip tests passed, and the full affected gate passed on the current branch.
