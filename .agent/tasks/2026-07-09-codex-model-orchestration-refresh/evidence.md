# Evidence: Codex model orchestration refresh

Verified at: `2026-07-09T20:53:37Z`

| AC  | Verdict | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1 | PASS    | `.agents/agents/codex-orchestrator.{md,toml}` and `.codex/agents/` map Sol to the parent, Spark to explorer/tool-support work, Luna to routine implementation, and Terra to escalation/review.                                                                                                                                                                                                                                                                                |
| AC2 | PASS    | `rg -n --hidden "gpt-5\.4-mini\|gpt-5\.4\|gpt-5\.5" .agents\agents .codex\agents .codex\hooks .codex\hooks.json` returned no matches.                                                                                                                                                                                                                                                                                                                                         |
| AC3 | PASS    | Project agents exist under the documented `.codex/agents/` location. Python `tomllib` parsed all project/legacy agent TOMLs and `.codex/config.toml`: `parsed json=1 toml=21`. Required agent files explicitly pin model and reasoning effort.                                                                                                                                                                                                                                |
| AC4 | PASS    | `.codex/hooks.json` registers `UserPromptSubmit` and `SubagentStart`. A read-only `codex exec` smoke with one-time reviewed-hook bypass returned exactly `parent=gpt-5.6-sol; search=gpt-5.3-codex-spark; implementation=gpt-5.6-luna; escalation=gpt-5.6-terra`, proving the hook reached model-visible developer context.                                                                                                                                                   |
| AC5 | PASS    | Representative `UserPromptSubmit` and `SubagentStart` JSON payloads were piped through the exact Windows hook command. Both outputs parsed as JSON and contained the expected `hookSpecificOutput.hookEventName`; the parent output contained all four routing lanes and the explorer output contained the read-only Spark reminder.                                                                                                                                          |
| AC6 | PASS    | JSON/TOML parsing passed, `git diff --check` passed, retired-model search was empty, and `codex doctor --json` reported `config.load: ok`, authenticated state, two locally consistent MCP servers, and reachable provider/WebSocket endpoints. The doctor's unrelated overall failure is only `TERM=dumb` in this non-interactive execution environment. Project-hook trust was not persisted or bypassed globally; the user must review the new hash once through `/hooks`. |

## Model availability proof

`codex debug models` listed these visible models in the current account/runtime:

- `gpt-5.6-sol` - latest frontier agentic coding model.
- `gpt-5.6-terra` - balanced agentic coding model for everyday work.
- `gpt-5.6-luna` - fast and affordable agentic coding model.
- `gpt-5.3-codex-spark` - ultra-fast coding model.
- `gpt-5.5`, `gpt-5.4`, and `gpt-5.4-mini` remain visible but are no longer used by active Kosmos orchestration configs.

`codex-auto-review` also exists but is hidden and reserved for automatic approval review.

## Notes

- The official Codex documentation says project hooks live in `.codex/hooks.json` and project custom agents in `.codex/agents/`.
- Hooks require hash-based trust review after creation or modification. The verification used `--dangerously-bypass-hook-trust` for one read-only smoke only and did not persist a trust decision.
- `debug prompt-input` does not execute lifecycle hooks, so it was not used as the final runtime proof.
