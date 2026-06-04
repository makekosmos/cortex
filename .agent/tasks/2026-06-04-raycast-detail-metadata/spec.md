# Spec - Raycast Detail.Metadata host slice

## Classification

FULL_LOOP. This extends Raycast root view support and renderer UI primitives in
the shell host.

## Goal

Trusted Raycast-compatible view commands can render root `Detail` views and
`Detail.Metadata` content in the Kosmos Raycast host.

## Acceptance Criteria

- Root `Detail` snapshots route to the Raycast detail renderer.
- `Detail.Metadata` does not pollute markdown fallback text.
- Metadata labels, links, separators, and tag lists are extracted into a typed
  host model.
- Metadata renders with Kosmos visual tokens and escaped text.
- Unit tests cover snapshot normalization and host metadata model.
- Visual verification covers root Detail + metadata layout.
- Docs mention the supported boundary.

## Out of Scope

- Opening metadata links through a guarded shell IPC action.
- Full Raycast metadata accessory/icon/color parity.
- QuickLook and nested metadata actions.
