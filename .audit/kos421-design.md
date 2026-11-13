# KOS-421 design decision

## Chosen shape

Use candidate A's additive Hello offer and candidate B's connection-scoped state rules. Keep the current supported protocol set at `{1}`. `protocol_version` remains the version used for the current session: the initiator's value is its preferred supported version, and the responder's value is the selected version. The responder must select a value from the initiator's bounded `supported_protocol_versions` list.

Advertise a bounded, sorted list of `supported_capabilities`; the response carries the selected intersection. A missing offer means a legacy v1 peer with no capabilities. Unknown capability identifiers are ignored after size/count validation. No capability is authorization. Capability-dependent behavior requires a session proof over the complete offer and selection. Preserve the existing Hello HMAC for legacy peers and add a separate domain-separated negotiation HMAC. If the shared secret is absent, or either proof is missing, the session remains v1 without semantic capabilities.

Use an addressed `ProtocolRejected` message for an incompatible offer. Include sender and target device IDs so the shared relay socket can broadcast the refusal while every other device ignores it. Do not close a relay connection to reject one peer. Close only the direct WS or Iroh peer connection when the transport supports peer-scoped close. Old peers cannot decode the new refusal; they fail closed and may only report a generic disconnect.

Store one immutable negotiated session on each authenticated connection. Validate the responder's selected version and capabilities on the client before setting the connection ready or reading version vectors. A reconnect starts negotiation again. Never change the selection on an authenticated connection.

The `durable_receipt_ack_v1` capability controls a separate receipt ACK. The receiver sends it only after the inbox receipt transaction commits. The existing `LiveAck` remains the applied ACK and is sent only after application commits. New-capability senders resume feed delivery from `receipt_through`; legacy senders keep using `applied_through`. Pending payload stays in the receiver's bounded inbox and can be applied later. Receipt progress is not proof of application and must not authorize feed compaction.

## Compatibility and safety invariants

- With extension fields absent, the serialized v1 Hello and all existing v1 fixtures remain byte-for-byte unchanged.
- The signed envelope, its canonical signing bytes, and its strict unknown-field rejection do not change.
- The server rejects an invalid or disjoint offer before peer persistence, callbacks, vector exchange, inbox writes, feed reads, or apply.
- A capability is selected only when both peer proofs cover the same bounded offers and selection. Legacy or unauthenticated sessions use no semantic capabilities.
- Receipt ACK advances only `receipt_through`. Applied ACK advances both receipt and applied progress. Duplicate ACKs are idempotent; ACKs for unsent positions are rejected.
- A relay refusal is targeted by device ID and does not affect unrelated peers on its shared socket.
- A peer outside the supported set gets the same deterministic refusal body for the same offer. A legacy peer may only observe connection closure.

## Required code shape

- A pure parser/selector owns version and capability validation, canonical offer bytes, and typed outcomes.
- The protocol model owns bounded optional Hello fields, the response selection fields, receipt ACK, and targeted refusal.
- Client and responder admission store a `NegotiatedSession`, not independent version/capability booleans.
- WS, Iroh, and relay adapters deliver the same typed selection. Relay uses sender/target IDs; Iroh and direct WS keep peer-scoped close behavior.
- SQLite receipt/applied cursor semantics remain in the existing committed-feed port. The wire adapter chooses the cursor from the negotiated capability.

## Review and verification

Candidate A scored 21/25 and candidate B scored 17/25 in `/tmp/arena-kos421/judgment.md`. The cross-judge's required corrections are included above: symmetric client selection validation, targeted relay refusal, bounded inputs, no capability use without a transcript proof, and the receipt/applied behavior already assigned to KOS-421 by `docs/ark-exocortex.md`.

The implementation is not complete until old/new fixture tests pass, every transport rejects before apply, the two ACK states have production integration coverage, the signed canonical fixture stays unchanged, and the ArkService JSON fixtures remain unchanged.
