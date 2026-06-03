# Fast task workflow

## Task

User request: "Обдумай как это все лучше сделать без потери качества и распредели задачи между агентами" followed by "делай".

Implement a lighter workflow for small Kosmos tasks so visual/UI fixes and other low-risk changes avoid full proof-loop ceremony while preserving explicit quality gates. Keep the existing full proof-loop for substantial work.

## Scope

- Add a task classification layer: `NO_LOOP`, `LIGHT_LOOP`, `FULL_LOOP`.
- Add local agent guidance for the classification step and light review path.
- Add targeted tooling for single-extension build/dev loops.
- Add reusable visual verification guidance/commands for fast UI tasks.
- Update docs-site source docs and generated agent context.
- Preserve existing substantial-task proof-loop roles and behavior.

## Non-goals

- Do not remove or weaken full proof-loop requirements for substantial work.
- Do not change ARK/data/sync/focus-mode safety rules.
- Do not introduce release/version bumps.
- Do not refactor extension architecture.
- Do not require full e2e or full smoke for every `LIGHT_LOOP` task.

## Acceptance Criteria

**AC1.** Documentation defines `NO_LOOP`, `LIGHT_LOOP`, and `FULL_LOOP`, including escalation triggers from `LIGHT_LOOP` to `FULL_LOOP`.

**AC2.** Existing proof-loop agent roles remain reserved for `FULL_LOOP`, and new local agent guidance exists for task classification before proof-loop creation.

**AC3.** Targeted extension tooling supports building one or more Vue extensions without building every extension or native release target.

**AC4.** Targeted extension dev tooling is exposed through package scripts, reusing the existing `dev-extensions.mjs --only` capability.

**AC5.** Visual fast-path guidance exists for UI tasks, including screenshot artifacts under `.tmp/` and a final report of what was and was not visually verified.

**AC6.** Docs source changes are synced into generated root context, and docs freshness checks pass.

## Verification Plan

- Inspect docs and agent files for the workflow definitions and role boundaries.
- Run targeted build command for a single extension if dependencies are available.
- Run docs sync and docs check.
- Run a smoke command for script parsing/help where possible.
