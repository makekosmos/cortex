# Eden: Open Library enrichment

## Classification

`FULL_LOOP`: a new external metadata read crosses the extension preload/main-process boundary and changes the book import workflow.

## Goal

Use a directly entered, already saved, or public-page-extracted ISBN to fill book metadata from Open Library without an API key.

## Scope

- Dedicated fixed-origin Open Library ISBN lookup in Electron main.
- Edition metadata: title, ISBN, cover, pages, language, publisher, publication date and author names when available.
- Book language is selected from predefined Russian labels; common ISO codes and language names normalize to the matching option.
- Best-effort merge into the existing preview; source-page values win.
- No new dependency, API key, ARK RPC, database/schema change, or background synchronization.

## Acceptance criteria

- **AC1.** Renderer can request only a validated ISBN through a typed preload method; it cannot choose the Open Library origin or arbitrary API URL.
- **AC2.** The main-process lookup calls `/isbn/{isbn}.json`, follows author keys only on the same fixed origin, has a timeout and response-size cap, returns `null` for 404, and maps supported edition fields.
- **AC3.** The import modal accepts a public HTTPS link or ISBN and prefills an ISBN already stored on the book. Direct ISBN input uses Open Library without fetching a page; for links, source-page values win and Open Library fills only missing values.
- **AC4.** Open Library failure is best-effort: already extracted source-page metadata remains previewable and applicable.
- **AC5.** Unit tests cover mapping, fixed-origin author lookup, 404 and invalid ISBN. Browser tests cover direct/prefilled ISBN lookup, enrichment merge and offline fallback.
- **AC6.** Eden tests, desktop build, ARK write guard, docs checks and the existing visual flow pass.
