# Problems

## P1 — evidence formatting

- **Observed:** the first fresh verifier run reported `oxfmt --check` failure for `evidence.md`; all code, unit/browser, ARK and docs checks in that run passed.
- **Cause:** the evidence table layout did not match repository Markdown formatting.
- **Fix:** ran `rtk bunx oxfmt .agent/tasks/2026-07-13-eden-top-navigation/evidence.md`.
- **Reverify:** the complete changed-file format check and the remaining fresh verifier commands were rerun after the fix and passed.
