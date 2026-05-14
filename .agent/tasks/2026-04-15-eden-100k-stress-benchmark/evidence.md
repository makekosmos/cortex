# Evidence — Eden 100k typing stress benchmark baseline

## Scope
- `apps/eden/ts/package.json`
- `apps/eden/ts/playwright.config.ts`
- `apps/eden/ts/playwright.stress.config.ts`
- `apps/eden/ts/scripts/compareTypingStress.ts`
- `apps/eden/ts/tests/typing-stress.spec.ts`
- `.agent/tasks/2026-04-15-eden-100k-stress-benchmark/spec.md`
- `.agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts/baseline-typing-stress-120000.json`
- `.agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts/compare-baseline-vs-baseline.json`

## What changed
- Added a dedicated **stress-only Playwright config** so the heavy benchmark is excluded from normal `test:e2e` runs.
- Added a reproducible **100k+ mixed-content benchmark** for Eden typing/open path.
- The generated note is intentionally heavy and now includes:
  - headings
  - blockquotes
  - bullet lists
  - code blocks
  - wikilink-like paragraphs
  - hundreds of regular prose paragraphs
- Added a reusable **comparison script** for artifact-vs-artifact diffs on each iteration.

## Fresh verification
- `bun run lint` ✅
- `bun run build` ✅
- `EDEN_STRESS_CHARS=120000 EDEN_STRESS_LABEL=baseline bun run benchmark:typing-stress` ✅
- `bun run test:e2e` ✅ (`17 passed`; stress benchmark excluded from default suite)
- `bun run scripts/compareTypingStress.ts --baseline ... --current ... --output ...` ✅

## Canonical baseline artifact
- `.agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts/baseline-typing-stress-120000.json`

## Canonical baseline scenario
- target chars: `120000`
- actual chars: `120372`
- paragraphs: `348`
- content bytes: `149192`
- structure counts:
  - headings: `15`
  - blockquotes: `15`
  - bullet lists: `14`
  - code blocks: `14`
  - wikilink paragraphs: `35`

## Baseline metrics
- open time: `229.78 ms`

### Normal mode
- input-to-next-paint p95/p99: `22.3 / 25.3 ms`
- update-to-next-paint p95/p99: `4.3 / 6.3 ms`
- long tasks: `1` (max `52 ms`)

### Zen mode
- input-to-next-paint p95/p99: `15.6 / 17.1 ms`
- update-to-next-paint p95/p99: `12.4 / 13.9 ms`
- long tasks: `0`

## Comparison infrastructure
- Compare script:
  - `apps/eden/ts/scripts/compareTypingStress.ts`
- Example usage:
```bash
bun run scripts/compareTypingStress.ts \
  --baseline /abs/path/to/baseline-typing-stress-120000.json \
  --current /abs/path/to/after-next-pass-typing-stress-120000.json \
  --output /abs/path/to/compare-after-next-pass-vs-baseline.json
```
- Verified self-compare artifact:
  - `.agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts/compare-baseline-vs-baseline.json`

## How to re-run after the next perf pass
From `apps/eden/ts/`:
```bash
EDEN_STRESS_CHARS=120000 EDEN_STRESS_LABEL=after-next-pass bun run benchmark:typing-stress
```
Then compare:
```bash
bun run scripts/compareTypingStress.ts \
  --baseline /Users/kirill/Documents/projects/kosmos/.agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts/baseline-typing-stress-120000.json \
  --current /Users/kirill/Documents/projects/kosmos/.agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts/after-next-pass-typing-stress-120000.json \
  --output /Users/kirill/Documents/projects/kosmos/.agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts/compare-after-next-pass-vs-baseline.json
```

## Remaining risks
- This is still a synthetic benchmark, not a full production corpus replay.
- It is strong enough for iteration-to-iteration comparison, especially because the document shape is now intentionally mixed and heavy.
