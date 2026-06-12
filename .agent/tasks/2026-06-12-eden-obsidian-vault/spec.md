# Eden Obsidian vault import/export

## Context

Eden stores user writing as ARK objects: object metadata and typed passport fields in `propsJson`, editor body in `contentJson`. The user wants to import an Obsidian vault into Eden, export Eden objects back to Obsidian-friendly Markdown, and support photos/images as first-class objects and visible Markdown media on pages.

## Scope

- Add Eden UI actions for importing a folder of Obsidian Markdown files and exporting Eden objects to a folder.
- Preserve Obsidian-compatible Markdown with YAML frontmatter and `[[wikilinks]]` where possible.
- Add a built-in object type `Изображение` with standard object metadata plus image-specific visible fields for file size and resolution.
- Make Markdown image syntax render in the CodeMirror editor preview.
- Extend extension host/preload APIs only through declared Eden permissions and existing ARK APIs. Do not write directly to ARK SQLite.

Out of scope:

- Full Obsidian plugin compatibility.
- Binary file editing, image compression, OCR, EXIF parsing beyond basic resolution/size.
- Multi-vault management UI.
- Changing the frozen ARK schema.

## Acceptance Criteria

**AC1.** Eden can import an Obsidian vault folder by recursively reading `.md` / `.markdown` files, creating or updating ARK objects through the Eden API facade, and preserving each note's title, Markdown body, YAML/frontmatter fields, and basic `[[wikilink]]` references in object content/props without direct SQL writes.

**AC2.** Eden can export Eden objects to an Obsidian-friendly folder as Markdown files with YAML frontmatter that includes object id, object type, title, timestamps, visible passport props, and a Markdown body, using stable safe filenames.

**AC3.** Eden has a built-in `Изображение` object type whose passport supports at least file name, MIME type, byte size, width, height, resolution text, source path, and alt text, with byte size/resolution visible and technical fields hidden/read-only where appropriate.

**AC4.** Importing vault attachments/images creates `Изображение` objects for supported local image files referenced by Markdown image syntax or located under common Obsidian attachment folders, and imported notes keep working Markdown image references.

**AC5.** CodeMirror Markdown rendering in Eden displays image markdown (`![](...)`) inline on pages instead of leaving it as only raw text when not actively editing that token.

**AC6.** Relevant guards/build/tests pass: Eden unit tests for Markdown/Obsidian conversion, `ark:guard:writes`, docs freshness, and Eden extension build.
