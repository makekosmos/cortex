# Evidence

## Repair: real image asset files for Eden image/header metadata

- `products/eden/src/lib/obsidianVault.ts` now resolves image asset metadata from:
  - linked Eden image objects,
  - header image fields,
  - inline image metadata objects,
  - and Markdown body image refs.
- Copyable local assets are emitted as real `sourcePath` file-copy entries under `assets/` with best-effort original filenames/extensions from `file_name`, `source_path`, `image`, and `mime_type`.
- Exported Markdown/frontmatter is rewritten to point at the copied asset path when the asset is copyable.
- `assets/obsidian-asset-manifest.json` remains auxiliary for unresolved/skipped references only.

## Checks

- `cd products/eden && rtk bun test tests/obsidianVault.test.ts` — PASS, 12/12 tests.

## Residual limitations

- Remote/data URL images remain unresolved and are left unchanged with manifest metadata when applicable.
- Host-time filesystem read/copy failures are only known in Electron export; they are logged with a skipped-asset count rather than represented in the renderer-built payload.
