# Evidence

Verified at: 2026-06-08T18:39:43.8300622+03:00

## AC1

Verdict: PASS

Commands:

```powershell
gh repo view ksanrse/horologion --json nameWithOwner,isArchived,visibility,url,defaultBranchRef
```

Result:

```json
{
  "defaultBranchRef": { "name": "master" },
  "isArchived": true,
  "nameWithOwner": "ksanrse/horologion",
  "url": "https://github.com/ksanrse/horologion",
  "visibility": "PRIVATE"
}
```

Archived source commit pushed before archive:

```text
a215457aa29b3669dc5d216b7e6f706919ffba49
```

## AC2

Verdict: PASS

Commands:

```powershell
Test-Path incubator\horologion
Test-Path extensions\horologion
Test-Path .tmp\horologion-archive
```

Result:

```text
False
False
False
```

## AC3

Verdict: PASS

Command:

```powershell
rg -n "horologion|Horologion" --glob "!.git/**" --glob "!node_modules/**" --glob "!**/node_modules/**" --glob "!.tmp/**" .
```

Result: exit code 1, no matches.

## AC4

Verdict: PASS

Commands and results:

```powershell
node scripts/check-docs-freshness.mjs
```

Result: PASS, docs fresh and no stale references.

```powershell
node scripts/check-ark-write-boundaries.mjs
```

Result: PASS, ARK write boundary guard passed.

```powershell
node scripts/ark-smoke.mjs
```

Result: PASS, ARK smoke matrix passed. First sandbox run failed with `os error 5` on `target\debug\.cargo-lock`; escalated rerun passed.

```powershell
bun test tests/unit/extension-update-plan.test.ts
```

Result: PASS, 2 tests passed. First sandbox run failed with EPERM reading the test file; escalated rerun passed.

```powershell
.\node_modules\.bin\tsc.exe --noEmit
```

Workdir: `platform/desktop`

Result: PASS.

## Notes

- `bun run docs:sync` and `bun run --cwd platform/desktop typecheck` both reported `Script not found` in this environment despite scripts existing in `package.json`. Used underlying commands directly: `node scripts/sync-agents-docs.mjs` and `.\node_modules\.bin\tsc.exe --noEmit`.
- `node scripts/sync-agents-docs.mjs` initially hit sandbox EPERM writing `AGENTS.md`; escalated rerun passed and regenerated generated docs.
