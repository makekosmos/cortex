# KOS-424 address policy boundary

- [x] Ground: trace address-policy imports and existing behavior tests.
- [x] Sketch: compare at least two module ownership designs.
- [x] Agree: select a design from the evidence; no human checkpoint requested.
- [x] Implement: extract only the selected pure address-policy boundary.
- [x] Scrap: no repeated implementation friction appeared; the selected boundary held.
- [x] Verify: run address behavior tests, crate checks, format, and source-size gate.
- [x] Commit independently after checks pass.

## Synthesis

Selected `ark-net-policy` as a std-only leaf. It owns the existing public
address and interface-name rules without interface enumeration. `ark-core::net`
re-exports every existing public item, and current consumers keep their import
paths. This creates a named policy dependency for the planned `ark-net` while
keeping transport and host access out of the policy crate.

The crate candidate was the strongest design for the KOS-424 network boundary.
The alternative domain-crate candidate had a factual grounding error about the
current workspace: `ark-domain` already exists. Its separate point to defer
extraction absent a second current consumer remains valid; KOS-424's target
network crate supplies the future consumer and makes this leaf useful now.

Verification: all 11 moved policy tests passed, the external-style
`ark_core::net` compatibility test passed, `cargo check -p ark-core
--all-targets`, strict Clippy for `ark-net-policy` and `ark-core`, formatting,
source-size, and diff checks passed.
The full serial `ark-core` suite passed 510/510.
The comment audit removed explanatory comments that duplicated the parsing
branches. The policy and compatibility tests, strict Clippy, format, and
source-size gates passed again afterward.
