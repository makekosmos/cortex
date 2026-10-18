# ARK sync-session extraction design

- [x] Ground: inspect existing Core session loops, `ark-sync`, `ark-net`, and the Stage 2 boundary rationale.
- [x] Sketch: compare two independently reasoned ownership and interface designs.
- [x] Agree: synthesize the smallest boundary that meets the Stage 2 ownership target.
- [ ] Implement: move one complete, behaviorally testable session unit at a time.
  - [x] Replace server and client paired outbound-snapshot flags with the `ark-sync` progress state; preserve separate sent and acknowledged checks.
  - [x] Move outbound snapshot page sequencing into `ark-sync`; Core handles typed page and terminal-send effects.
  - [x] Move server-Hello authentication, space, and protocol-selection validation into `ark-sync`.
  - [x] Add bounded typed `SessionIo` in `ark-sync` and a supervised WebSocket adapter in `ark-net`; keep reader and writer workers independent under one joined owner.
  - [ ] Move the client connected-message loop behind typed session input and effects.
  - [ ] Move the server connected-message loop behind typed session input and effects.
- [ ] Scrap: re-evaluate if implementation repeatedly needs Core policy or concrete transport inside `ark-sync`.
