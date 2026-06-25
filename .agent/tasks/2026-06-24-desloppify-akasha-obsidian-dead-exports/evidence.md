# Evidence

## Commands

- `rg readEpubFile|readEpubBytes|InlineSpan|ObsidianImageObjectDraft|ObsidianVaultAssetManifestEntry|importObsidianVault|buildObsidianExportFiles incubator/akasha products/eden tests/unit docs-site`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension akasha`
- `bun run --cwd platform/desktop build:extension eden`
- `bun test tests/unit/akasha-epub-guardrails.test.ts products/eden/tests/obsidianVault.test.ts`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-akasha-obsidian-dead-exports/desloppify-after.json`

## Results

- References check: PASS.
  - Current Akasha reader/library code imports `readEpubBytes`, not
    `readEpubFile`.
  - `InlineSpan`, `ObsidianImageObjectDraft`, and
    `ObsidianVaultAssetManifestEntry` are only used inside their defining
    files.
  - Active public parser/import-export APIs remain exported.
- `typecheck`: PASS.
- `build:extension akasha`: PASS.
- `build:extension eden`: PASS.
- Focused tests: PASS, 15 tests across EPUB guardrails and Obsidian vault.
- Baseline scan: score 0, 460 findings; severity critical 0, high 259,
  medium 130, low 71.
- Final scan: score 0, 456 findings; severity critical 0, high 255,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 4 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Scoped dead exports are removed.
- AC3: PASS. Active parser/import-export APIs remain exported.
- AC4: PASS. Relevant checks passed.
