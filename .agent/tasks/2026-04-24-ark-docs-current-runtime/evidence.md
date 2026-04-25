# Evidence: Current ARK Runtime Documentation

Verification result: PASS

## Acceptance Criteria

- AC1: PASS. Root README links ARK to `packages/ark-core/README.md`; `doc-check.txt` confirms no legacy root ARK README link remains.
- AC2: PASS. `packages/ark-core/README.md` documents build/test commands, `ark-core-rpc`, and `@arksync/node` lifecycle/integration.
- AC3: PASS. Current limitations and direct-write policy are documented in `packages/ark-core/README.md`.
- AC4: PASS. `TODO.md` and `ARK-P2P-WORKPROGRESS.md` now start with legacy notes and point to the current ARK README.
- AC5: PASS. Documentation content checks and `git diff --check` passed.

## Raw Artifacts

- `doc-check.txt`
- `source-evidence.txt`
- `legacy-reference-grep.txt`
- `git-diff-check.txt`
- `git-diff.txt`

## Notes

- `legacy-reference-grep.txt` still contains one expected historical mention inside the legacy note itself; `doc-check.txt` verifies the active root link is current.
- `git diff --check` reports CRLF conversion warnings only and exits 0.
