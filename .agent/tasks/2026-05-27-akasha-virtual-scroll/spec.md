# 2026-05-27 — Akasha virtual scroll

## Goal

Remove scroll jank and CPU load from Akasha's continuous EPUB reader by rendering
only the visible block range.

## Scope

- Keep the user-facing continuous document model.
- Replace full-book render with GPUI/gpui-component virtual list rendering.
- Keep top chapter navigation working via virtual-list scroll target.
- Avoid rebuilding flattened reader blocks on every render.

## Acceptance Criteria

**AC1.** Akasha stores flattened reader blocks after EPUB load and does not clone
the full book into a new `Vec` on every render.

**AC2.** Reader body uses a virtualized GPUI component so only visible block
ranges are materialized as elements.

**AC3.** Chapter navigation still scrolls to the selected chapter's first block.

**AC4.** Existing EPUB parser and state tests stay green.
