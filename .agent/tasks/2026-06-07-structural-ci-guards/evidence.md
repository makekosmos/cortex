# Evidence: 2026-06-07 structural-ci-guards

Verified on branch `structural-refactor-ci-guards`.

The local branch first recorded the CI evidence, then merged
`origin/structural-refactor-ci-guards` after the remote moved ahead by 10
commits. The full check suite below was rerun after that merge.

## AC1

Verdict: PASS

Command:

```powershell
bun install --frozen-lockfile
```

Result: exited 0. Bun reported `Checked 930 installs across 1117 packages
(no changes)`.

## AC2

Verdict: PASS

Commands:

```powershell
bunx oxlint .
bunx oxfmt --check .
bun run ark:guard:writes
bun run docs:sync
bun run docs:check
```

Results:

- `bunx oxlint .` exited 0.
- `bunx oxfmt --check .` exited 0 and reported `All matched files use the correct format.`
- `bun run ark:guard:writes` exited 0 and reported `ARK write boundary guard passed.`
- `bun run docs:sync` exited 0 and regenerated `AGENTS.md`, `CLAUDE.md`, mobile/ARK AGENTS docs, and llms files.
- `bun run docs:check` exited 0 and reported `всё свежо, stale references не найдено`.

## AC3

Verdict: PASS

Commands:

```powershell
bun run --cwd platform/desktop typecheck
bun run --cwd core/ark/packages/ark typecheck
bun run --cwd core/ark/packages/ark test
bun run --cwd products/eden test:unit
```

Results:

- `platform/desktop typecheck` exited 0.
- `core/ark/packages/ark typecheck` exited 0.
- `core/ark/packages/ark test` exited 0: 12 tests passed.
- `products/eden test:unit` exited 0: 20 tests passed.

## AC4

Verdict: PASS

Command:

```powershell
cargo clippy --workspace --all-targets
```

Result: exited 0. Clippy produced warnings only; CI command does not deny those warnings.

## AC5

Verdict: PASS

Commands:

```powershell
bun run shell:build
bun run ark:smoke
```

Results:

- `bun run shell:build` exited 0; desktop renderer/main/preload builds completed.
- `bun run ark:smoke` exited 0 and reported `ARK smoke matrix passed`.

## AC6

Verdict: PASS

Final status/staging review completed after merging the remote branch. The local
checkout still contains untracked old-layout cache directories (`shell/`,
`extensions/eden/`) with no tracked files; they are intentionally excluded from
staging and push.
