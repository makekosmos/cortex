# Codex Hooks On Windows

## Trigger

Use when adding or debugging a project `.codex/hooks.json` command hook on Windows, especially when the hook should inject developer context.

## Symptom

- A direct hook-script test says `hook_event_name is required` even though the test command appears to contain JSON.
- `codex debug prompt-input` does not show the expected lifecycle-hook context.
- A hook works from the repo root but its command is fragile when launched through a different Windows shell.

## Do This

1. Keep `commandWindows` free of PowerShell variables that an outer PowerShell may expand. Resolve the repo root inline:

   ```text
   powershell -NoProfile -ExecutionPolicy Bypass -Command "& (Join-Path (git rev-parse --show-toplevel) '.codex\hooks\hook.ps1')"
   ```

2. Pipe JSON from a process that emits exact bytes, then JSON-parse the hook output:

   ```powershell
   rtk proxy node -e "process.stdout.write(JSON.stringify({hook_event_name:'UserPromptSubmit'}))" |
     rtk proxy powershell -NoProfile -ExecutionPolicy Bypass -Command "& (Join-Path (git rev-parse --show-toplevel) '.codex\hooks\hook.ps1')"
   ```

3. For an end-to-end model-visible-context smoke, run one read-only `codex exec` after reviewing the hook source. A one-off `--dangerously-bypass-hook-trust` may verify an already-vetted hook without persisting trust; do not use it as the normal launch path.

4. Persist normal trust through `/hooks`. Codex hashes hook definitions, so edits require a new review.

## Avoid

- Do not treat `codex debug prompt-input` as proof that lifecycle hooks executed; use a real read-only turn for runtime verification.
- Do not embed `$root` or similar variables inside a nested `powershell -Command` string launched from PowerShell unless escaping is proven.
- Do not automatically write hook hashes into global config or make bypass flags permanent.

## Promote To Skill When

Promote this to a skill when multiple repos need a shared hook authoring, trust, and runtime-verification workflow.
