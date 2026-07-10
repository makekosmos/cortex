# Codex model orchestration refresh

## Goal

Update the Kosmos Codex orchestration contract for the locally available GPT-5.6 model family and connect it to Codex lifecycle hooks so the routing policy is present during real project work, not only in a manually opened orchestrator file.

## Scope

- Refresh the existing `.agents/agents/` orchestration prompts and model pins.
- Add current project-scoped Codex custom-agent files under `.codex/agents/` where needed for runtime discovery.
- Add a project-local lifecycle hook that injects the routing contract as developer context.
- Keep `gpt-5.3-codex-spark` as the fast read/search/tool-support lane for medium/high-complexity tasks.
- Use `gpt-5.6-luna` for routine bounded implementation and `gpt-5.6-terra` for escalation after Luna is insufficient or for deeper independent verification.
- Use `gpt-5.6-sol` as the parent/control-plane model.

## Non-goals

- No product/runtime code changes.
- No version bump, release, push, or global default-model change.
- No heuristic prompt classifier based on keywords or prompt length.
- No automatic trust bypass for project hooks.
- No removal of legacy `.agents/agents/` files that existing repo documentation still references.

## Acceptance criteria

**AC1.** The refreshed orchestration contract explicitly maps Sol to parent/control-plane work, Spark to read/search/tool-support work on medium/high tasks, Luna to routine implementation, and Terra to escalation/deeper verification.

**AC2.** Active orchestration and proof-loop role configs no longer pin `gpt-5.4-mini`, `gpt-5.4`, or `gpt-5.5`; their model pins and escalation text use the intended Sol/Spark/Luna/Terra lanes.

**AC3.** Current Codex can discover project-scoped custom agents from `.codex/agents/`, including explicit model and reasoning settings for the parent, explorer, routine worker, Terra escalation worker, reviewer, and proof-loop roles.

**AC4.** A project-local hook configured in `.codex/hooks.json` injects concise developer context on `UserPromptSubmit`; it requires the parent to classify task complexity and, for medium/high tasks, delegate read/search/tool-support work to Spark while routing implementation Luna -> Terra escalation.

**AC5.** The hook command is Windows-compatible, reads the documented JSON payload from stdin, and emits valid JSON with `hookSpecificOutput.hookEventName` and `additionalContext` without mutating repository state.

**AC6.** Fresh verification confirms JSON/TOML syntax, hook execution output, absence of retired model references in active agent configs, and a healthy Codex configuration check. Any hook trust requirement is reported explicitly rather than bypassed.

## Verification plan

- Parse `.codex/hooks.json` and every `.codex/agents/*.toml` file.
- Invoke the hook script with representative `UserPromptSubmit` and `SubagentStart` payloads and validate its JSON output.
- Search active `.agents/agents/` and `.codex/agents/` configs for retired model slugs.
- Run `codex doctor --summary` against the refreshed project configuration.
- Record per-criterion evidence in `evidence.md` and `evidence.json`, then rerun the verification commands for the final verdict.
