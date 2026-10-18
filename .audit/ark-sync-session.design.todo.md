# ARK sync-session extraction design

- [x] Ground: inspect existing Core session loops, `ark-sync`, `ark-net`, and the Stage 2 boundary rationale.
- [x] Sketch: compare two independently reasoned ownership and interface designs.
- [x] Agree: synthesize the smallest boundary that meets the Stage 2 ownership target.
- [ ] Implement: move one complete, behaviorally testable session unit at a time.
  - [x] Replace server and client paired outbound-snapshot flags with the `ark-sync` progress state; preserve separate sent and acknowledged checks.
  - [x] Move outbound snapshot page sequencing into `ark-sync`; Core handles typed page and terminal-send effects.
  - [x] Move server-Hello authentication, space, and protocol-selection validation into `ark-sync`.
  - [x] Add bounded typed `SessionIo` in `ark-sync` and a supervised WebSocket adapter in `ark-net`; keep reader and writer workers independent under one joined owner.
  - [x] Wire the Core client loop to typed bounded session input/output and remove the Core-owned unbounded WebSocket writer task; keep Core protocol and admission handling in place for the next extraction.
  - [x] Run bulk snapshot production outside the receive loop and give ACK/admission messages a bounded priority queue so bulk backpressure does not suspend protocol reads.
  - [x] Verify authenticated peer identity is used for registry-policy admission, including a discovered `seed-*` identity; verify backpressure and teardown with focused tests and targeted clippy.
  - [x] Replace the server's unbounded WebSocket message queues with typed bounded `SessionIo`; keep outbound admission and signed-frame authorization at the socket write boundary.
  - [ ] Move the client connected-message loop behind typed session input and effects.
  - [ ] Move the server connected-message loop behind typed session input and effects.
- [ ] Scrap: re-evaluate if implementation repeatedly needs Core policy or concrete transport inside `ark-sync`.
