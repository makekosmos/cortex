# Task: makekosmos release channel bridge

Date: 2026-06-07

Branch: `codex/makekosmos-release-channel`

Classification: FULL_LOOP

## Goal

Move Kosmos desktop and extension distribution to the dedicated `makekosmos`
GitHub organization, while shipping one bridge desktop release to both the old
and new desktop updater repositories.

## Acceptance Criteria

### AC1 — GitHub distribution repos

- `makekosmos/desktop` exists and is public.
- `makekosmos/extensions` exists and is public.
- Previously created placeholder repos are not left under the wrong names.

### AC2 — Desktop updater bridge

- `platform/desktop/package.json → build.publish` has primary provider
  `makekosmos/desktop`.
- `yoso-industries/kepler-releases` remains as a secondary bridge provider for
  this migration release.
- Kosmos Desktop patch version is bumped from `0.4.2` to `0.4.3`.

### AC3 — Extension marketplace channel

- `extension-marketplace.ts → CATALOG_URL` points to
  `https://raw.githubusercontent.com/makekosmos/extensions/main/catalog.json`.
- `publish-extension.mjs` and `generate-catalog.mjs` use
  `makekosmos/extensions`.

### AC4 — Documentation

- Distribution docs describe `makekosmos/desktop`, `makekosmos/extensions`, and
  the temporary `yoso-industries/kepler-releases` bridge target.
- Root docs are synced from `docs-site/`.
- `STATUS.md` and `docs-site/whats-new/kepler.md` mention the `0.4.3` bridge
  release.

### AC5 — Verification

- `bun install --frozen-lockfile`
- `bunx oxlint .`
- `bunx oxfmt --check .`
- `bun run ark:guard:writes`
- `bun run docs:sync`
- `bun run docs:check`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:js:shell`
- Release artifacts are built and published to both
  `makekosmos/desktop` and `yoso-industries/kepler-releases`, or a concrete
  blocker is recorded.

## Notes

The old `yoso-industries/kepler-releases` channel must not be removed in this
task. Existing installed clients need it to discover the bridge update.
