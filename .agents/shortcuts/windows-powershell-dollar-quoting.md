# Windows PowerShell Dollar Quoting

## Trigger

When running `powershell -Command` from Codex and the command contains `$env:...`, `$_`, or other PowerShell variables.

## Symptom

PowerShell receives `.Name` instead of `$_.Name`, or `='.tmp\...'` instead of `$env:CARGO_TARGET_DIR='...'`, because the outer shell expanded `$...` before PowerShell saw it.

## Do This

Use a single-quoted PowerShell command string:

```powershell
rtk proxy powershell -NoProfile -Command '$env:CARGO_TARGET_DIR=".tmp\cargo-check"; cargo check --manifest-path platform/runtime/Cargo.toml --lib'
```

For process queries:

```powershell
rtk proxy powershell -NoProfile -Command 'Get-CimInstance Win32_Process | Where-Object { $_.Name -in @("cargo.exe","rustc.exe","link.exe") } | Select-Object ProcessId,Name,CommandLine'
```

## Avoid

Do not wrap PowerShell commands containing `$` in outer double quotes unless every `$` is escaped for the invoking shell.

## Promote To Skill When

This starts affecting broader Windows command recipes beyond Kosmos build/debug loops.
