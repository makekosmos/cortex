# Evidence — fast task workflow

Verified on 2026-06-03.

## AC1 — PASS

Documentation defines `NO_LOOP`, `LIGHT_LOOP`, and `FULL_LOOP` in `docs-site/concepts/proof-loop.md`.

Evidence:

- `docs-site/concepts/proof-loop.md` now contains `## Классификация задач`.
- The same section defines escalation triggers from `LIGHT_LOOP` to `FULL_LOOP`.
- `docs-site/agents/claude-md-core.md` mirrors the compact boot-context version.

## AC2 — PASS

Existing proof-loop agent roles remain reserved for `FULL_LOOP`, and pre-proof-loop classification guidance exists.

Evidence:

- Added `.agents/agents/task-classifier.md`.
- Added `.agents/agents/task-classifier.toml`.
- Updated `task-spec-freezer`, `task-builder`, `task-verifier`, and `task-fixer` MD/TOML files to reserve them for `FULL_LOOP`.

## AC3 — PASS

Targeted extension tooling supports building one or more Vue extensions without building every extension or native release target.

Evidence:

- `shell/scripts/build-extensions.mjs` supports `--only`, `--only=`, `--changed`, `--vue-only`, and `--skip-native`.
- `bun run --cwd shell build:extensions:only eden --skip-native` passed and built only Eden, then printed `no native extensions to build`.
- `bun run --cwd shell build:extensions:changed` passed and built affected Vue extensions, then printed `no native extensions to build`.

## AC4 — PASS

Targeted extension dev tooling is exposed through package scripts.

Evidence:

- `shell/package.json` now has `dev:extension` and `dev:extensions:only`.
- `shell/scripts/dev-extensions.mjs` supports `--only=...` and fails on unknown/non-dev ids.
- `node shell/scripts/dev-extensions.mjs --only=__missing__` returned exit 1 with `unknown or non-dev Vue extension id(s): __missing__`.

## AC5 — PASS

Visual fast-path guidance exists for UI tasks.

Evidence:

- `.agents/skills/visual-verify/SKILL.md` now documents `.tmp/visual/<YYYY-MM-DD>-<task-slug>/<surface>-<viewport>-<state>.png`.
- `docs-site/agents/checklists.md` now has `LIGHT_LOOP` and `NO_LOOP` checklists.
- Root package scripts now expose `visual:regression`, `visual:launcher`, and `visual:eden`.
- No UI surface was changed in this task, so no screenshot artifact was required for this proof-loop.

## AC6 — PASS

Docs source changes are synced into generated root context, and docs freshness checks pass.

Evidence:

- `bun run docs:sync` passed.
- `bun run docs:check` passed with `всё свежо, stale references не найдено`.

## Notes

Some local Node/Bun commands required sandbox escalation because the managed Windows sandbox failed before script execution with `windows sandbox: setup refresh failed`. The commands executed only local repository scripts and did not require network access.
