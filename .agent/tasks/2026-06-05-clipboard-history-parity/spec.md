# Clipboard History Raycast Parity

Started: 2026-06-05
Class: FULL_LOOP

## Context

Before merging `codex/raycast-compat-runtime` into `main`, the local Clipboard History surface should cover the Raycast-like core workflow instead of only listing recent text/images.

External reference checked on 2026-06-05:

- Raycast Clipboard History page: supports text, images, colors, links, files, search/filter, pinning, copy/open/remove/clear, and a detail view with metadata.
- Raycast API docs: clipboard APIs include reading previous history by offset, and copy actions can opt out of history.

## Scope

In scope:

- Current in-memory clipboard history implementation in Electron shell.
- Text, image, link, color, and file-like entries where Electron exposes the content.
- Pin/unpin, delete, clear non-pinned, clear all, copy, open, and detail metadata.
- Launcher embedded clipboard mode and standalone clipboard history view.
- Unit coverage for store behavior.

Out of scope:

- Encrypted persistence.
- Ignore-app preferences and password-manager detection.
- OS-level paste into the previously focused app.
- OCR/text extraction from images.

## Acceptance Criteria

**AC1.** Clipboard store models Raycast-like item types: text, image, link, color, and file, with stable ids, timestamps, search text, optional metadata, and pinned state.

**AC2.** Clipboard capture classifies copied text into text/link/color, captures images, and captures file-like clipboard paths when Electron exposes them.

**AC3.** Clipboard actions support copy, delete, pin/unpin, clear non-pinned, clear all, and open where applicable. Pinned entries stay at the top and survive normal clear.

**AC4.** Launcher clipboard mode exposes all type filters, search, pin/unpin, delete, copy/open, clear non-pinned, and metadata without breaking command mode.

**AC5.** Standalone Clipboard History view exposes the same actions and shows a detail/metadata panel for the selected entry.

**AC6.** Relevant unit tests, shell typecheck, lint, and ARK write-boundary guard pass.

**AC7.** Branch is merged into `main` only after AC1-AC6 pass.
