# Evidence

Baseline scan:

- findings: 297
- high: 140
- medium: 113
- low: 44

After scan:

- findings: 292
- high: 138
- medium: 110
- low: 44

Targeted Knip result:

- `png-to-ico` cleared
- `rcedit` cleared
- `sharp` cleared
- `electron/preload.ts` cleared
- `electron/extension-preload.ts` cleared

Notes:

- `workspaces["platform/desktop"].project` stayed `electron/**/*.ts`
- remaining noise still includes unrelated exports and files
