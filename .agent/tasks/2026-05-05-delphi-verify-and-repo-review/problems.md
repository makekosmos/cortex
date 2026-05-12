# Problems

None — all four ACs PASS against current code.

Items below are non-blocking advisories (no fixer pass scheduled):

1. CRLF/LF warnings across nearly every modified file under Windows working tree. Pin via `.gitattributes` if not already done. Cosmetic — does not affect ACs.
2. Electron downgrade `^41.1.0 → ^38.8.4` in `apps/delphi/ts/package.json`. Vitest + tsc PASS, but packaged build was not exercised here — that lives in `.agent/tasks/2026-04-27-packaged-smoke/`.
3. `apps/eden/ts/main/store.ts` issues an additional `list_object_types` RPC on every `listArkObjects` / `getArkEntry` call. Acceptable now; revisit if type-set or call frequency grows.
4. Arrancador surface (~+850/-211 across `electron/main/services/*`) was not exercised by this task. Out of scope per spec. Recommend rerunning arrancador vitest + the new `apps/arrancador/scripts/run-packaged-smoke.ts` before tagging a release.
