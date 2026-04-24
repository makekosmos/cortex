# Evidence

## Scope

This task cleaned up `game_obj` presentation in Eden so game notes surface only useful player-facing metadata by default, while system/install metadata stays in the object data model.

## Implemented Changes

- Updated built-in `game_obj` defaults in:
  - `apps/eden/ts/src/lib/systemTypes.ts`
  - `packages/ark-core/rust/src/db.rs`
  - `apps/eden/ts/scripts/seedArkObjectDemo.ts`
- Added automatic Eden-side normalization for legacy noisy `game_obj` presets:
  - `apps/eden/ts/main/store.ts`
  - `apps/eden/ts/src/store/eden.ts`
- Added shared human-readable field formatting:
  - `apps/eden/ts/src/lib/objectFieldFormatting.ts`
- Updated typed object rendering:
  - `apps/eden/ts/src/components/typed-notes/TypedHeader.vue`
  - `apps/eden/ts/src/components/typed-notes/ObjectPropertyField.vue`
  - `apps/eden/ts/src/components/objects/TypeObjectsView.vue`

## Behavior Changes

### `game_obj` default presentation

The default Eden header/list presentation now focuses on:

- `play_status`
- `genres`
- `total_playtime_seconds`
- `last_played_at`

The following fields are now hidden from the normal header presentation:

- `description`
- `user_rating`
- `cover_image`
- `background_image`
- `related_notes`
- `exe_path`
- `save_path`
- `play_count`
- `save_exists`
- `rawg_id`
- `exe_name`
- `sync_source`

### Automatic handling for existing spaces

If an existing `game_obj` type still uses the old noisy built-in preset, Eden now upgrades that presentation in-memory automatically without requiring manual user intervention.

This normalization is intentionally narrow:

- it applies only to `game_obj`
- it applies only when the stored UI schema still matches the old built-in preset
- it does not overwrite arbitrary user customizations

### Header cleanup

- Hidden `background_image` values no longer leak into the header background.
- The fallback type icon tile is no longer shown when there is no visible cover.

### Human-readable field formatting

- `play_status` now renders as readable labels.
- `total_playtime_seconds` now renders as readable playtime.
- `last_played_at` now renders as a readable Russian date.
- Type collection rows use the same formatting.

## Verification

### PASS

- `cargo check --bin ark-core-rpc`
  - artifact: `artifacts/cargo-check.txt`
- `bun x tsc --noEmit`
  - artifact: `artifacts/tsc.txt`
- `bun x vite build -c vite.config.mjs --configLoader native`
  - artifact: `artifacts/vite-build.txt`
  - note: Vite completed successfully; Bun/PowerShell surfaced warnings through a non-zero wrapper exit, but the build output itself completed

### Environment note

- `bun run build`
  - artifact: `artifacts/build.txt`
  - note: blocked by a locked `ark-core-rpc.exe` file in the local environment, not by a compile error in the modified code

## Acceptance Criteria Status

- AC1: PASS
- AC2: PASS
- AC3: PASS
- AC4: PASS
- AC5: PASS
- AC6: PASS

## Encoding Check

- New proof-loop spec/evidence files were written in ASCII to avoid mojibake.
- New code additions in `objectFieldFormatting.ts` use ASCII-safe unicode escapes for user-facing labels where needed.
