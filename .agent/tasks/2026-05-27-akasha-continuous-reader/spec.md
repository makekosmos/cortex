# 2026-05-27 — Akasha continuous reader

## Goal

Turn Akasha from a chapter dump viewer into a native EPUB reading surface: one
continuous text flow, preserved basic document structure, and chapters available
from a top control instead of a permanent sidebar.

## Scope

- Preserve EPUB block semantics for headings, paragraphs, list items, and
  blockquotes.
- Preserve basic inline emphasis for bold and italic text.
- Render all spine content as one continuous vertically scrolling document.
- Replace the permanent chapter sidebar with a top chapters control whose panel
  can scroll independently.
- Keep the existing native extension contract and local reader state behavior.

## Acceptance Criteria

**AC1.** Akasha parses fixture EPUB XHTML into structured reader blocks in spine
order, including heading levels and inline bold/italic marks.

**AC2.** The reader view renders one continuous document containing all loaded
chapters; users do not need to click the next chapter to keep reading.

**AC3.** The permanent left chapter sidebar is removed. A top chapters control
opens a scrollable chapter panel, and selecting a chapter moves the reader to
that chapter while preserving continuous reading after it.

**AC4.** Headings, paragraphs, list items, blockquotes, bold text, and italic
text have distinct visual treatment in GPUI.

**AC5.** Scrolling work is kept out of render-time state churn: text scrolling
uses a tracked GPUI scroll surface and chapter navigation updates its offset
directly instead of re-rendering one selected chapter at a time.

**AC6.** Existing Akasha state persistence and native-extension headless launch
behavior remain intact.
