# desloppify Knip false-positive entry/ignore cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-delphi-qrcode-dependency-cleanup/desloppify-after.json`
- score 9, total 218, HIGH 70, MEDIUM 105, LOW 43

After:

- `.agent/tasks/2026-06-24-desloppify-knip-false-positive-entry-ignore-cleanup/desloppify-after.json`
- score 9, total 205, HIGH 57, MEDIUM 105, LOW 43

Rule delta:

- `DEAD_FILE`: 19 -> 6

Config changes:

- Marked manual scripts as root entries.
- Marked `site/src/main.ts` as the marketing site runtime entry.
- Ignored unsupported/generated false positives for Vite configs, ambient global typings, and generated ARK TS output.

Rejected earlier:

- Root entry for `site/vite.config.ts` lowered `DEAD_FILE` but introduced site-local Vite `UNLISTED_DEPENDENCY` findings and dropped score.

Checks:

- temp Knip comparisons showed no added file/dependency/unlisted noise.
- `desloppify scan --json` produced the after JSON; exit 1 is expected while findings remain.
- mojibake grep over `knip.json` passed.
