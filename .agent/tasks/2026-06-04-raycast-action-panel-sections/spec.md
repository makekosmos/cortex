# Raycast ActionPanel Sections

## Goal

Raycast-compatible `ActionPanel.Section` nodes must be rendered as visible grouped action areas in the Kosmos Raycast host while preserving existing action execution behavior.

## Acceptance Criteria

- The host model exposes section-aware action groups with titles for `ActionPanel.Section`.
- Loose root actions still render when no section is present.
- Existing `actionNodes()` callers and execution logic continue to work.
- The action panel UI shows section titles and grouped buttons without nested interactive elements.
- Unit tests and visual verification cover a mixed root-action plus section-action panel.
- Docs and proof evidence reflect the new compatibility surface.
