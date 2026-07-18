# Evidence — Codewars integration

Verified on 2026-07-18 against the current worktree.

## AC1 — integration tile and settings

**PASS.** `tests/e2e/integrations-body.spec.ts` passed against the freshly built alternate backend. It verifies the fourth connected tile, modal copy, username field, and disabled sync before connection.

Visual inspection: the four providers form one bordered block with 1 px dividers, and the locally bundled Codewars mark is rendered from the official Codewars SVG.

Artifact: `../2026-07-17-integrations-body/raw/integrations-dashboard.png`.

## AC2 — full then incremental import without duplicates

**PASS.** Rust tests cover the stable username-qualified object ID, completion mapping, profile mapping, and overlap cutoff. A live test-slot synchronization against the public `Snowfrogdev` profile completed with `lastError: null` and `importedCount: 61` in the isolated `tests/.e2e/codewars-live-smoke/integrations.json`.

The provider credential update resets the previous sync cursor, so changing usernames performs a full first import for the new profile.

## AC3 — separate cached Codewars statistics

**PASS.** `tests/e2e/coder.spec.ts` passed. It verifies Codewars rank, honor, language list, kata list, stable tab position, and no extra ARK reads after dashboard navigation.

Artifact: `raw/coder-codewars.png`.

## AC4 — truthful fields and links

**PASS.** Rust mapping test verifies `https://www.codewars.com/kata/<slug>` and confirms no solution code is stored. The Codewars table omits LeetCode-only number, runtime, and memory columns. The UI and docs explicitly state that the public API does not expose failed attempts or per-kata difficulty in the completed list.

## AC5 — final checks

**PASS.** Commands:

```text
CARGO_TARGET_DIR=.tmp/cargo-codewars-test cargo test -p kepler-backend integrations::tests
bun test platform/desktop/src/coder/useCoderStats.test.ts              # 5 passed
bun run --cwd platform/desktop typecheck                               # PASS
bun run --cwd platform/desktop build:js:shell                          # PASS
bun run ark:guard:writes                                               # PASS
bunx playwright test tests/e2e/coder.spec.ts ...                       # 1 passed
bunx playwright test tests/e2e/integrations-body.spec.ts --grep ...    # 1 passed
bun run docs:sync
bun run docs:check                                                     # PASS
git diff --check                                                       # PASS
```

The default Cargo target was not used because a running workspace `target/debug/kepler-backend.exe` held the Windows file lock. Verification used an isolated target directory and passed without stopping the user's dev process.
