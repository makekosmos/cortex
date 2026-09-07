# Arcadia installed-principal contract handoff

- Test-only harness revision: `origin/main` plus this change; no production runtime changes.
- Catalog: sequence `15`, previous `14`, Store sequence `14`; publish workflow `34056365312` SUCCESS.
- Arcadia version: `0.1.11`; catalog-15 artifact size `2746695`; SHA-256 `c2b820511c97caf26696cbeb5db15f31c98617fac7cadc976162b5357f0f9f5c`.
- Producer ref/tag: `7b68da25e5dd2a4433a764eb89c7ca4a2001f253` / `v0.1.11`.
- Delivered backend: `C:\mk\.worktrees\cortex-gate\desktop\release\win-unpacked\resources\Kosmos Runtime.exe`; size `46870528`; SHA-256 `1911258c116a6c34c9ad9b777b5bf001546e993b17a69ba5807e01b6a40a2023`.

## Actual acceptance

The headless test applied the authenticated downloaded catalog-15 document and
signatures, installed and enabled the exact Arcadia archive, launched the
installed principal, and verified:

- `games.list`: HTTP 200, `{ok:true}`.
- `delete_object`: HTTP 403, `{ok:false,error:"forbidden"}`.
- Result: `1 pass, 0 fail, 0 skip`.

The fixture is test-only and is not a trust bypass. Hosted CI and Store refresh
smoke are separate acceptance items; Store refresh remains blocked by the
published Store-14/index-15 package skew.
