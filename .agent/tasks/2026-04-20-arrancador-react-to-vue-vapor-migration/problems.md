# Problems

## 1. Composite `bun run build` is still unstable in this local Bun/Windows setup

Observed status:

- `bun run build:renderer` passes when invoked directly.
- `bun run build:main` passes.
- `bun run build:preload` passes.
- `bun run build` still fails while invoking `vite build` from the composite script path.

Observed failure signature:

- `Could not load @tailwindcss/oxide-win32-x64-msvc`
- `spawn EPERM`

Assessment:

- This is a tooling/orchestration issue in the local Bun + Windows + Vite/Tailwind path.
- The migrated Vue renderer itself is not blocked, because direct renderer build succeeds against the current codebase.

Smallest safe workaround today:

- use the verified direct steps instead of the wrapper:
  - `bun run predev`
  - `bun run build:renderer`
  - `bun run build:main`
  - `bun run build:preload`

## 2. Legacy React source tree still exists as inactive reference code

Observed status:

- `apps/arrancador/src/` still exists in the repository.
- The active runtime, primary Vite config, primary typecheck path, and default tests no longer depend on it.

Assessment:

- This does not block the migration acceptance criteria.
- It is a cleanup opportunity only, not a runtime blocker.
