# Evidence — Kosmos integrations, body view, and packaged runtime correctness

## Verdict

`PASS` — every acceptance criterion in the frozen spec is satisfied by the current worktree.

## Root cause and fix

The installed `0.6.25` runtime and the stale default `target/release/kepler-backend.exe` are
byte-identical (same size and SHA256), while that backend predates the calculator change. The
release workflow built the fresh backend under an alternate `CARGO_TARGET_DIR`, but Electron read
the hard-coded default target. The build now stages binaries from the effective Cargo target and
Electron packages only the staged paths. A deterministic Node test proves the alternate-target
artifact becomes `Kosmos Runtime.exe`.

## Acceptance criteria

| Criterion                    | Verdict | Current evidence                                                                                                                                                        |
| ---------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1 packaged runtime         | PASS    | Alternate-target staging test `1/1`; calculator/currency E2E PASS                                                                                                       |
| AC2 common integrations page | PASS    | Dashboard cards open the shared Hevy/Toggl controls in `@kosmos/visuals` `Modal`; the old settings entry is removed; E2E PASS                                           |
| AC3 credential safety        | PASS    | Hevy key validated HTTP 200, stored in Windows Credential Manager, renderer receives only `hasCredential`; exact-secret scan has zero matches                           |
| AC4 Hevy import              | PASS    | Official workouts/templates/events contract, deterministic ARK IDs, update/delete handling; unit tests PASS; isolated live sync imported 166 records and was cleaned up |
| AC5 Toggl import             | PASS    | Official v9 Basic Auth and `/me/time_entries`, running/deleted mapping, deterministic ARK IDs; contract/mapping tests PASS                                              |
| AC6 scheduling               | PASS    | Per-provider validated presets/startup flags, global non-overlap guard, credential skip, readiness-safe spawned scheduler                                               |
| AC7 body experience          | PASS    | Direct `kosmos:body` command, built-in dashboard section, accessible front/back MIT-attributed muscle maps, development and week/month/year load E2E/screenshots PASS   |
| AC8 fresh proof              | PASS    | Rust `451 passed, 5 ignored`; desktop build/typecheck; relevant E2E `4/4` plus command registration `1/1`; format/lint/ARK/docs/diff gates PASS                         |

## Artifacts

- `raw/verification-output.txt`
- `raw/integrations-dashboard.png`
- `raw/integrations-modal-toggl.png`
- `raw/body-development.png`
- `raw/body-load-month.png`
- `problems.md` records the failed hidden-window screenshot attempt and unrelated baseline test
  behavior, together with the clean reverification results.

The body screenshots are deterministic E2E fixtures, not the user's live Hevy history. Production
`BodyView` calls `integrations.body_snapshot`, which reads imported `workout_obj` records from ARK;
no sample workout is present in the production path.

## Follow-up verification — shared sidebar and dashboard integrations

- Desktop typecheck: PASS.
- Desktop shell build: PASS.
- Visuals unit tests: `86/86` PASS.
- Integrations/body E2E: `2/2` PASS.
- Settings About E2E after promoting the settings shell into the main `Sidebar`: `1/1` PASS.
- Visual inspection: dashboard cards, Toggl modal, and Settings About sidebar PASS.

## External contracts used

- Hevy Swagger: `https://api.hevyapp.com/docs/`
- Hevy credential page: `https://hevy.com/settings?developer`
- Toggl authentication: `https://engineering.toggl.com/docs/track/authentication/`
- Toggl time entries: `https://engineering.toggl.com/docs/track/api/time_entries/`
- Muscle polygons: `GV79/react-body-highlighter`, MIT notice preserved beside the adapted data.

No version, tag, release, installer upload, or installed application was replaced; distribution is
intentionally deferred until the user explicitly authorizes a release.
