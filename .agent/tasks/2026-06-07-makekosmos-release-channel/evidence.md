# Makekosmos Release Channel Bridge Evidence

Date: 2026-06-07
Branch: `codex/makekosmos-release-channel`
Classification: `FULL_LOOP`

## Summary

Kosmos Desktop `0.4.3` is published as a bridge release:

- New desktop updater channel: `makekosmos/desktop`.
- Temporary legacy updater channel: `yoso-industries/kepler-releases`.
- New extension marketplace channel: `makekosmos/extensions`.
- The packaged app's `app-update.yml` points to `makekosmos/desktop`, so clients that install `0.4.3` should use the new channel for future updates.
- The old `yoso-industries/kepler-releases` release remains available so already-installed clients can discover this bridge update.

## Acceptance Criteria

| AC                                 | Verdict | Evidence                                                                                                                                                                                                                                              |
| ---------------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1. GitHub distribution repos     | PASS    | `makekosmos/desktop` and `makekosmos/extensions` exist and are public. Previously created placeholder repos were renamed to `desktop` and `extensions`. `makekosmos/desktop` was initialized with a minimal README so GitHub releases can be created. |
| AC2. Desktop updater bridge        | PASS    | `platform/desktop/package.json` uses `makekosmos/desktop` as the first publish provider and keeps `yoso-industries/kepler-releases` as the second provider. Version is bumped from `0.4.2` to `0.4.3`.                                                |
| AC3. Extension marketplace channel | PASS    | Runtime catalog URL and publish/catalog scripts point to `makekosmos/extensions`. Migrated catalog and existing extension release assets are available in `makekosmos/extensions`.                                                                    |
| AC4. Documentation                 | PASS    | `STATUS.md`, `docs-site/whats-new/kepler.md`, and distribution docs describe the `0.4.3` bridge and the temporary yoso channel. `docs:sync` and `docs:check` passed.                                                                                  |
| AC5. Verification and release      | PASS    | Local checks passed. `v0.4.3` was built with the alternate Cargo target workaround and published to both desktop updater repos with installer, blockmap, and `latest.yml`.                                                                            |

## Verification Commands

```powershell
bun install --frozen-lockfile
bun run docs:sync
bunx oxlint .
bun run ark:guard:writes
bun run docs:check
bun run --cwd platform/desktop typecheck
bun run --cwd platform/desktop build:js:shell
bunx oxfmt platform/desktop/electron/extension-marketplace.ts
bunx oxfmt --check .
cargo build --release --manifest-path Cargo.toml --bin kepler-backend --bin ark-core-rpc --bin kepler-focus-helper --bin kepler-focus-svc
bun run --cwd platform/desktop build:js
bun run --cwd platform/desktop stage:bundled-extensions
bunx electron-builder --win nsis --publish always -c .tmp/electron-builder-bridge.json
gh release view v0.4.3 -R makekosmos/desktop --json tagName,url,assets,isDraft,isPrerelease
gh release view v0.4.3 -R yoso-industries/kepler-releases --json tagName,url,assets,isDraft,isPrerelease
Get-Content platform/desktop/release/latest.yml
Get-Content platform/desktop/release/win-unpacked/resources/app-update.yml
```

## Release Evidence

`makekosmos/desktop`:

- Release: https://github.com/makekosmos/desktop/releases/tag/v0.4.3
- `Kosmos-Setup-0.4.3.exe`, size `114088624`, sha256 `2224eb331ef76f7c781cf10adcddc6ee27bc552427be8b56a6528c582aff801a`
- `Kosmos-Setup-0.4.3.exe.blockmap`, size `118398`, sha256 `f7582c8ce73231db4f340ab40c492b475a9e16aa86a7ba082b95565b3e007329`
- `latest.yml`, size `341`, sha256 `bc664fd374dca288d6dad8b758a9108f125256ac9422fc84c3dc4f02315a35a0`

`yoso-industries/kepler-releases`:

- Release: https://github.com/yoso-industries/kepler-releases/releases/tag/v0.4.3
- `Kosmos-Setup-0.4.3.exe`, size `114088624`, sha256 `2224eb331ef76f7c781cf10adcddc6ee27bc552427be8b56a6528c582aff801a`
- `Kosmos-Setup-0.4.3.exe.blockmap`, size `118398`, sha256 `f7582c8ce73231db4f340ab40c492b475a9e16aa86a7ba082b95565b3e007329`
- `latest.yml`, size `341`, sha256 `bc664fd374dca288d6dad8b758a9108f125256ac9422fc84c3dc4f02315a35a0`

`latest.yml` verification:

```yaml
version: 0.4.3
path: Kosmos-Setup-0.4.3.exe
size: 114088624
releaseDate: "2026-06-07T20:25:50.022Z"
```

Packaged updater config verification:

```yaml
owner: makekosmos
repo: desktop
provider: github
releaseType: release
updaterCacheDirName: kepler-shell-updater
```

## Extension Marketplace Migration

The new `makekosmos/extensions` repository contains migrated extension releases:

- `akasha-v0.1.2`
- `arrancador-v0.1.4`
- `delphi-v0.1.7`
- `eden-v0.1.12`
- `horologion-v0.1.7`

The initial `catalog.json` was migrated from the old yoso catalog so Horologion remains visible even though it is not in the current local bundled extension build set.

## Environment Notes

The normal desktop build initially hit the known Windows service lock on `target\release\kepler-focus-svc.exe`. The release build used the documented workaround:

1. Build Rust binaries into `.tmp\cargo-release`.
2. Generate a temporary `platform/desktop/.tmp/electron-builder-bridge.json`.
3. Run `electron-builder` with that temporary config.

No permanent package config was changed for this workaround.
