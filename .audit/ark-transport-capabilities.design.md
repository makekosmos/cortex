# ARK transport capability boundary

## Decision

Keep `ark-sync` transport-neutral and split its current transport contract into a mandatory lifecycle/broadcast contract and one required Iroh capability contract. `ark-core` owns the closed `SyncTransportHandle::{Relay, Iroh}` selection because it is the composition and orchestration layer. Transport-specific code must match the enum before calling Iroh-only operations; do not expose `require_iroh() -> Result` as a substitute for that match.

The Iroh variant carries one object implementing a composed `IrohSessionTransport` contract. Keep the composed contract narrow enough to describe current Iroh pairing, addressed send, admission, and endpoint close operations, but do not split it into multiple ports unless a real independent implementation or ownership boundary needs them. Relay implements only common lifecycle and broadcast. No default methods that return "unsupported" strings remain.

Construct Iroh with its outbound frame authorizer already installed. The authorizer is mandatory, immutable transport state. Remove the late mutable setter and ignored configuration result. Define peer disconnect as local trust/state removal plus a separately reported network effect: Iroh closes and suppresses a peer endpoint; Relay keeps its shared WebSocket and reports that no single-peer network close occurred.

## Compatibility rules

The transport choice must not change wire encoding or existing Engine API. Preserve current Relay pairing and legacy protocol behavior until compatibility tests prove a deliberate new offer profile. In particular, Relay currently advertises `ProtocolOffer::current(...)`, while Hello handling expects `RegistryHistoryV1` in one path. Capture the current intended outcomes in tests before deriving offers from capabilities. Registry coordinator creation and use must be based on observed call paths, not an assumption that it is Iroh-only.

## Implementation order

1. Add tests for Relay Hello negotiation and per-peer disconnect with the shared socket intact; retain Iroh admission, authorization, addressed-send and endpoint-suppression tests.
2. Introduce typed transport error/outcome types and remove fake defaults by splitting the contracts.
3. Add the closed handle in `ark-core` and migrate startup plus the common lifecycle first.
4. Migrate addressed sends, registry exchange, pairing and disconnect one call path at a time, matching the Iroh variant at each boundary.
5. Require the authorizer in Iroh construction, then run focused Relay/Iroh tests and workspace gates before committing.

## Alternatives and tradeoffs

- A large trait implemented by both Relay and Iroh preserves one dynamic dispatch point but falsely claims Relay supports Iroh admission and addressed delivery. Reject it.
- Independent optional capability trait objects allow mismatched Relay/Iroh combinations and require repeated presence checks. Reject it.
- A closed enum alone does not make a specific operation statically callable; the exhaustive `match` at the operation boundary is the guarantee. Accept one explicit branch at those boundaries.
- A generic `RelaySync<T>` propagates transport types through coordinator and service state with only two active backends. Reject the additional type surface.
- Keeping authorization mutable after construction permits an Iroh instance to run without the security dependency. Reject it; constructor requires the authorizer.

## Judge reconciliation

The independent judge preferred the smaller closed-family form, while retaining constructor-required authorization and explicit disconnect outcomes from the other candidate. The judge also corrected two assumptions: Relay pairing is handled by shared `RelaySync`, and Relay's current registry coordinator is constructed unconditionally, so its actual Relay call paths must be established before relocating it. These corrections are reflected in the implementation order above.

## Acceptance evidence

This design is accepted only when tests prove Relay compatibility, Iroh's required authorization/admission behavior, and honest disconnect outcomes; the source contains no default unsupported capability methods or ignored authorizer setup; and targeted checks pass on the resulting commit.
