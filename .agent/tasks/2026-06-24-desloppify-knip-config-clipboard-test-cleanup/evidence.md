# desloppify Knip config + clipboard test cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-delphi-qrcode-dependency-cleanup/desloppify-after.json`
- score 9, total 218, HIGH 70, MEDIUM 105, LOW 43

After:

- `.agent/tasks/2026-06-24-desloppify-knip-config-clipboard-test-cleanup/desloppify-after.json`
- score 9, total 202, HIGH 57, MEDIUM 102, LOW 43

Rule deltas:

- `DEAD_FILE`: 19 -> 6
- `WEAK_ASSERTION`: 23 -> 19
- `SLEEPY_TEST`: 27 -> 28

Changes:

- Added manual script entries and `site/src/main.ts` to Knip root entries.
- Ignored known unsupported/generated Knip file false positives: Vite configs, ambient global typing, and generated ARK TS output.
- Tightened `clipboard-history-store.test.ts` assertions from null/truthy checks to explicit item field checks.
- Made the persistence test wait for the store's existing 400ms debounced disk write.

Checks:

- temp Knip comparisons showed `files 19 -> 6` with no added dependency/devDependency/unlisted noise.
- `bun test tests/unit/clipboard-history-store.test.ts` passed.
- `bun run --cwd platform/desktop typecheck` passed.
- mojibake grep over touched files passed.
- `desloppify scan --json` produced the after JSON; exit 1 is expected while findings remain.

Tradeoff:

- The clipboard persistence test now exposes the existing debounced persistence timing, adding one `SLEEPY_TEST`; net result is still three fewer findings and four fewer weak assertions.
