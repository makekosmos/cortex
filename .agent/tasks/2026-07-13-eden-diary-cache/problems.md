# Problems

## P1 — browser test formatting

- **Observed:** the first changed-file `oxfmt --check` rejected `BubbleDiaryView.spec.ts`.
- **Cause:** the newly added fixture test had an unformatted long statement.
- **Fix:** ran `rtk bunx oxfmt products/eden/tests/components/BubbleDiaryView.spec.ts`.
- **Reverify:** the changed-file format check and full verifier are rerun after the fix.

## P2 — evidence formatting

- **Observed:** the first fresh verifier rejected `evidence.md`; all product checks in the same run passed.
- **Cause:** the evidence table did not match repository Markdown formatting.
- **Fix:** ran `rtk bunx oxfmt .agent/tasks/2026-07-13-eden-diary-cache/evidence.md`.
- **Reverify:** the complete verifier is rerun after the fix.
