# Problems

## P1: Vite build is blocked by sandbox process restrictions

- Symptom:
  - `bun run build` for both `apps/delphi/ts` and `apps/eden/ts` fails before renderer bundling completes.
- Current evidence:
  - Vite fails during config/dependency resolution with `spawn EPERM` from the Windows `externalize-deps` path.
  - Delphi additionally fails to load the Tailwind oxide native module in this environment.
- Why this is not fixed in-code here:
  - The failure occurs in environment-level process spawning/native dependency loading before the modified Vue shell code is actually bundled.
  - `tsc --noEmit` for both apps passes, so the changed TypeScript/Vue wiring is at least type-valid.
- Smallest safe next step:
  - Re-run `bun run build` locally outside this sandbox on the same workspace.
  - If it still fails outside sandbox, capture the new build output and treat it as a separate dependency/toolchain issue.
