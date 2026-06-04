---
name: windows-sandbox
description: Windows Sandbox recovery rules for Codex in Kosmos. Use when shell commands fail with sandbox setup/permission errors, Windows path/glob behavior differs from bash, or verification/build commands need a clean rerun without wasting retries.
---

# Windows Sandbox

## Fast Recovery

- If an important command fails with `windows sandbox: setup refresh failed with status exit code: 1`, rerun the same command once with `sandbox_permissions: "require_escalated"` and a short justification. Do not spend tokens trying equivalent commands first.
- Keep escalation scoped: use the narrowest safe `prefix_rule` such as `["bun","run","shell:build"]`, `["bunx","playwright","test"]`, or a specific script path. Do not request broad `python`, destructive, or arbitrary shell prefixes.
- If a network/install/build/test command fails with DNS, registry, permission, or sandbox-looking errors, rerun with escalation before diagnosing product code.
- If a command uses heredoc/herestring, broad scripting, or destructive operations, do not add `prefix_rule`; ask only for the one command when needed.

## PowerShell Gotchas

- Do not rely on bash-style globs inside arguments. `rg extensions/*/manifest.json` can fail on Windows with `os error 123`. Prefer `rg --files -g manifest.json extensions` or `Get-ChildItem -Path extensions -Recurse -Filter manifest.json`.
- Prefer native PowerShell cmdlets end-to-end for filesystem operations. Do not enumerate paths in PowerShell and pass them to `cmd /c` for deletion or moving.
- Before recursive delete/move, verify resolved absolute paths stay inside `D:\Personal\Hobby\Coding\kosmos` or the explicitly intended target directory.
- Avoid command separators for routine reads; run separate tool calls or `multi_tool_use.parallel` so output stays readable and approvals stay scoped.

## Verification Notes

- Electron e2e commonly runs against `shell/dist-electron`. After editing `shell/electron/*.ts`, run `bun run shell:build` before Playwright if the test launches the built app.
- `shell/playwright.config.ts` uses `shell/e2e`; root tests live under `tests/e2e` and should use root `playwright.config.ts`.
- When rerunning after an escalated sandbox failure, record the original failure and the successful escalated rerun in proof-loop evidence.
