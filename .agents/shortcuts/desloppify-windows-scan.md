# Desloppify Windows Scan

## Trigger

When running `bunx desloppify scan --json .` from PowerShell on Windows.

## Symptom

`desloppify` fails with `Executable not found in $PATH: "which"` or the JSON
written by PowerShell `>` cannot be parsed by Node because it is UTF-16.

## Do This

Create a temporary shim and run the scan through `cmd` redirect:

```powershell
New-Item -ItemType Directory -Force .tmp\bin | Out-Null
Set-Content -Encoding ascii .tmp\bin\which.cmd '@where.exe %*'
cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-current.json"
```

## Avoid

Do not use PowerShell `>` for the final JSON artifact; it writes UTF-16 in this
environment. Do not treat exit code 1 as scan failure when JSON was written;
the CLI returns non-zero when findings exist.

## Promote To Skill When

This becomes part of regular audit or release workflow.
