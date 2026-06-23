# Windows Desloppify Via Bunx

## Trigger

When running `desloppify` on Windows through `bunx`.

## Symptom

`bunx desloppify ...` fails with `Executable not found in $PATH: "which"`.

## Do This

Use Git for Windows `which.exe` for that command:

```powershell
rtk proxy cmd /c "set PATH=C:\Program Files\Git\usr\bin;%PATH%&& bunx desloppify scan --summary --json . > .tmp\desloppify-summary.json"
```

For staged scans:

```powershell
rtk proxy cmd /c "set PATH=C:\Program Files\Git\usr\bin;%PATH%&& bunx desloppify scan --staged --summary --json . > .tmp\desloppify-staged-summary.json"
```

## Avoid

Do not create a repo-local `which` wrapper just for a scan. Do not trust focused `--category` names across desloppify versions without checking `desloppify scan --help`.

## Promote To Skill When

This becomes part of the regular audit/release checklist or more Windows-only `desloppify` failures show up.
