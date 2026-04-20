# Evidence - Kepler shared typography tokens

## Summary
A shared primary title typography token set was added to `kepler-visuals`, and Eden first-level headings were migrated to use it instead of local hardcoded values.

## Code Evidence
- `packages/kepler-visuals/theme/css-variables.css`
  - introduced:
    - `--kepler-type-title-1-size`
    - `--kepler-type-title-1-line-height`
    - `--kepler-type-title-1-letter-spacing`
    - `--kepler-type-title-1-weight`
- `apps/eden/ts/src/Editor.css`
  - note/object title input now reads the shared token
- `apps/eden/ts/src/components/settings/SettingsPage.css`
  - settings page titles now read the shared token
- `apps/eden/ts/src/components/spaces/SpacesView.css`
  - page hero headers now read the shared token
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeEditor.vue`
  - object type editor primary title now reads the shared token
- `apps/eden/ts/src/App.css`
  - duplicate legacy/global title-input rules were aligned to the same shared token so they cannot override with stale values

## Verification

### AC1
PASS

Shared primary title typography token set exists in `kepler-visuals`.

### AC2
PASS

Eden note/object title input uses the shared title token.

### AC3
PASS

Eden settings page titles use the shared title token.

### AC4
PASS

Other first-level Eden headings on the same visual tier now use the shared title token as well.

### AC5
PASS

Checks passed:
- `bun x tsc --noEmit`
- `bun run build`

Raw artifacts:
- `raw/tsc.txt`
- `raw/build.txt`
