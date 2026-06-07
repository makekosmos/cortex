# 2026-06-07 structural-ci-guards

## Context

Branch `structural-refactor-ci-guards` moves the repository layout from the old
`shell` / `extensions` / `crates` roots to `platform`, `products`, `incubator`,
and `core/ark`. The CI workflow must pass against the new paths before the
branch can be pushed or merged.

## Scope

In scope:

- Fix the current CI blocker on `structural-refactor-ci-guards`.
- Verify the branch using the commands declared in `.github/workflows/ci.yml`.
- Run the wider local smoke/build checks that cover the moved desktop and ARK
  paths.
- Commit and push the fixed branch.

Out of scope:

- Merging the previously reviewed `codex/raycast-compat-runtime` branch, which
  has separate runtime blockers.
- Product/UI behavior changes unrelated to CI/path correctness.
- Direct release/version bump.

## Acceptance Criteria

AC1. `bun install --frozen-lockfile` succeeds without changing tracked files.

AC2. CI guard commands pass: `bunx oxlint .`, `bunx oxfmt --check .`,
`bun run ark:guard:writes`, and `bun run docs:check`.

AC3. CI type/unit commands pass: `bun run --cwd platform/desktop typecheck`,
`bun run --cwd core/ark/packages/ark typecheck`,
`bun run --cwd core/ark/packages/ark test`, and
`bun run --cwd products/eden test:unit`.

AC4. Rust CI command `cargo clippy --workspace --all-targets` exits with code 0.

AC5. Wider local smoke/build commands pass: `bun run shell:build` and
`bun run ark:smoke`.

AC6. The final git diff contains only intentional tracked changes for this
task, and the fixed branch is pushed to GitHub.

## Verification commands

- `bun install --frozen-lockfile`
- `bunx oxlint .`
- `bunx oxfmt --check .`
- `bun run ark:guard:writes`
- `bun run docs:check`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd core/ark/packages/ark typecheck`
- `bun run --cwd core/ark/packages/ark test`
- `bun run --cwd products/eden test:unit`
- `cargo clippy --workspace --all-targets`
- `bun run shell:build`
- `bun run ark:smoke`

## Out of scope decisions

The local checkout contains untracked old-layout build/cache directories
(`shell/`, `extensions/eden/`). They are not tracked in this branch and are not
part of the fix; they must not be committed.
