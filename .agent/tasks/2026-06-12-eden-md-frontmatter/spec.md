# Eden Markdown Frontmatter Import/Export

## Context

Eden stores user content as typed objects. The product direction is:

- Every note-like item is an object; there are no untyped notes.
- ARK/Eden object data remains the canonical runtime source of truth.
- Markdown files are an Obsidian-compatible import/export representation.
- One object should map to one `.md` file for the initial implementation.
- Object relationships should be represented with Obsidian-style `[[wikilinks]]` where possible.

## Scope

In scope for this task:

- Add a pure Eden Markdown frontmatter codec that can export one Eden entry/object into `.md` text.
- Add a pure parser that can import one `.md` text into an Eden entry draft shape using YAML frontmatter for object passport fields and Markdown for body content.
- Add a manual UI/API path for exporting the current Eden object and importing a Markdown file.
- Keep writes inside existing Eden save APIs / ARK boundary.
- Preserve the current CodeMirror/Vim editor behavior for body editing.

Out of scope for this task:

- Live filesystem sync / file watchers.
- Conflict resolution between ARK and files.
- Bulk folder import/export.
- Full object-link graph sync.
- Editing YAML frontmatter directly inside the Eden editor UI.
- New schema migration or destructive data changes.

## Acceptance Criteria

**AC1.** Eden has a pure Markdown frontmatter codec module that converts an `Entry` plus available `NoteType` metadata into a `.md` string with YAML frontmatter and a Markdown body, without performing ARK writes or filesystem writes.

**AC2.** The exported Markdown frontmatter includes stable Eden metadata (`id`, `type`, `schema_version`) and user-facing passport fields (`title`, tags/links when available), while the body remains standard Markdown suitable for Obsidian.

**AC3.** Eden can parse one `.md` file with YAML frontmatter into an entry draft suitable for saving through existing Eden APIs. Unknown frontmatter fields are preserved in entry metadata where the current model supports it, or explicitly ignored with a documented limitation in code comments.

**AC4.** Eden exposes a manual import/export UI path in Russian for the current object, using safe preload/Electron APIs rather than renderer filesystem access.

**AC5.** Import/export writes do not use direct SQL and do not bypass the existing Eden save boundary (`window.api` / kepler API shim / `@kosmos/ark` path).

**AC6.** The implementation does not replace ARK as source of truth and does not add live round-trip sync behavior.

**AC7.** Relevant verification is recorded in `evidence.md` / `evidence.json`, including at least static checks for changed TypeScript/Vue files and ARK write-boundary guard if platform/desktop or Eden data APIs are touched.
