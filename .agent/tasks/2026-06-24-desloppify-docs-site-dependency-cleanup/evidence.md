# Docs Site Dependency Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-visuals-dependency-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `184`
- Severity: `HIGH 51`, `MEDIUM 91`, `LOW 42`
- `DEAD_DEPENDENCY`: `13`

## Change

- Removed unused `docs-site` dev dependencies:
  - `@braintree/sanitize-url`
  - `cytoscape`
  - `cytoscape-cose-bilkent`
- Ran `bun install` to refresh `bun.lock`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-docs-site-dependency-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `181`
- Severity: `HIGH 51`, `MEDIUM 88`, `LOW 42`
- `DEAD_DEPENDENCY`: `10`

## Checks

- `rtk proxy rg -n "@braintree/sanitize-url|sanitizeUrl|cytoscape|cose-bilkent|Ð|Ñ|Рџ|�|Â|â€|Ã" docs-site --glob '!node_modules/**' --glob '!public/full-llms.txt'`
- `rtk bun install`
- `rtk err bun run docs:check`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-docs-site-deps.json"`
