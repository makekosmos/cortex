# Evidence

## Commands

- `rg repoExtensionRoots|REPO_EXTENSION_ROOT_NAMES|readManifestSafe|nativeExecutablePath|readManifestFromZip platform products packages core tests scripts .github docs-site`
- `rg repo-extension-roots\.mjs|extension-package-utils\.mjs|zip-utils\.mjs platform products packages core tests scripts .github docs-site`
- `node -e "const roots=await import('./platform/desktop/scripts/repo-extension-roots.mjs'); const pkg=await import('./platform/desktop/scripts/extension-package-utils.mjs'); const zip=await import('./platform/desktop/scripts/zip-utils.mjs'); ..."`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension delphi`
- `KOSMOS_HEADLESS=1 KOSMOS_TEST_MODE=1 bunx playwright test --config platform/desktop/playwright.config.ts platform/desktop/e2e/kext-install.spec.ts platform/desktop/e2e/kext-argv.spec.ts platform/desktop/e2e/kext-revert.spec.ts`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-desktop-script-dead-exports/desloppify-after.json`

## Results

- References check: PASS.
  - `REPO_EXTENSION_ROOT_NAMES`, `repoExtensionRoots`, and `readManifestSafe`
    are only used inside `repo-extension-roots.mjs`.
  - `nativeExecutablePath` is only used inside `extension-package-utils.mjs`.
  - `readManifestFromZip` had no external references and no internal callers.
  - Public script APIs imported elsewhere remain exported.
- Import smoke: PASS.
  - `repo-extension-roots.mjs` exports `findRepoExtensionEntry` and
    `listRepoExtensionEntries`.
  - `extension-package-utils.mjs` exports `buildNativeRelease`, `fileSize`,
    `packageExtensionKext`, and `sha256File`.
  - `zip-utils.mjs` exports `entriesFromDir`, `extractZip`, `readZip`,
    `safeEntryName`, and `writeZip`.
- `typecheck`: PASS.
- `build:extension delphi`: PASS.
- Focused kext e2e: PASS.
- Baseline scan: score 0, 501 findings; severity critical 0, high 300,
  medium 130, low 71.
- Final scan: score 0, 496 findings; severity critical 0, high 295,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 5 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Confirmed dead script exports are removed.
- AC3: PASS. Active script APIs remain exported and importable.
- AC4: PASS. Relevant checks passed.
