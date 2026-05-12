# Raw command results: Packaged smoke checks for Arrancador and Dashboard

## Arrancador typecheck

Command:

```powershell
bun run --cwd apps/arrancador typecheck
```

Result: PASS

## Dashboard typecheck

Command:

```powershell
bun run --cwd apps/dashboard typecheck
```

Result: PASS

## Dashboard Electron smoke

Command:

```powershell
bun run --cwd apps/dashboard test:e2e:smoke
```

Result: PASS after outside-sandbox rerun. The smoke seeded `apps/dashboard/.e2e/smoke-dashboard.db`, launched Electron, verified the connected status, top app, scrolling, sidebar resize, and sessions route.

## Arrancador packaged smoke

Command:

```powershell
bun run --cwd apps/arrancador smoke:packaged
```

Result: PASS after outside-sandbox rerun and smoke-only builder config adjustment.

Observed success payload:

```json
{
  "status": "ok",
  "executablePath": "D:\\Personal\\Hobby\\Coding\\kepler\\apps\\arrancador\\release\\win-unpacked\\arrancador.exe",
  "userData": "D:\\Personal\\Hobby\\Coding\\kepler\\apps\\arrancador\\.e2e\\packaged-smoke\\localappdata\\arrancador",
  "arkDbPath": "D:\\Personal\\Hobby\\Coding\\kepler\\apps\\arrancador\\.e2e\\packaged-smoke\\ark\\ark.db",
  "title": "Arrancador",
  "bodyLength": 241
}
```

## diff check

Command:

```powershell
git diff --check
```

Result: PASS. Git printed only LF/CRLF warnings for modified files.
