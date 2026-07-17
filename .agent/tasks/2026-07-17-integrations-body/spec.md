# Kosmos integrations, body view, and packaged runtime correctness

## Classification

`FULL_LOOP` — the change crosses desktop shell UI, the Rust runtime, external API imports,
ARK writes, background scheduling, credential storage, and Windows packaging.

## Goal

1. Fix the Windows packaging path that shipped the 0.6.25 calculator UI with the older
   backend, leaving both calculator and daily currency conversion unavailable.
2. Add a common dashboard integrations page with working Hevy and Toggl Track importers.
3. Add a shell-owned (not extension-owned) body page with strength development and muscle
   load views driven by imported Hevy workouts.

## Scope

In scope:

- stage freshly built Rust binaries from the active Cargo target directory before Electron
  packaging, including builds that use `CARGO_TARGET_DIR` to avoid locked production files;
- Hevy and Toggl Track cards using one common settings model: credential, official credential
  link, polling interval, sync on Kosmos startup, current status, and manual sync;
- provider-specific history window settings;
- credentials stored only in the OS keyring; the user-provided Hevy key is configured locally
  and never written to this repository, task artifacts, screenshots, or logs;
- Hevy workout/template import and deletion propagation into ARK objects;
- Toggl Track time-entry import and deletion propagation into ARK `time_entry_obj` objects;
- a bounded, non-overlapping background scheduler that honors each provider's interval and
  startup setting;
- a built-in dashboard body page with `Развитие` and `Нагрузка` subpages, editable body weight,
  front/back muscle maps, and week/month/year load ranges;
- muscle SVG geometry adapted from an MIT-licensed existing component with attribution;
- targeted unit/E2E checks, visual screenshots, write-boundary checks, typecheck/build, and a
  fresh verifier pass.

Out of scope:

- publishing, deleting, or replacing a GitHub release; bumping a desktop version or tag requires
  a separate explicit user command;
- OAuth flows (both initial providers use API credentials);
- bidirectional writes back to Hevy or Toggl;
- medical claims or clinically validated muscle-development scoring;
- real-time webhooks, multi-account credentials, historical currency quotes, or a generic plugin
  SDK for arbitrary future providers.

## Product decisions

- Integration settings are device-local because credentials and schedules are device-local.
- Imported domain records use ARK RPC writes so normal ARK sync/version-vector behavior remains
  intact; renderers never write SQLite directly.
- Hevy uses `api-key` authentication and links to `https://hevy.com/settings?developer`.
- Toggl uses HTTP Basic authentication with `<api-token>:api_token` and links to
  `https://track.toggl.com/profile`.
- Strength is a transparent game score based on estimated 1RM divided by body weight and a small
  exercise/muscle benchmark table. Load is set tonnage, with secondary muscles receiving half
  credit. The UI labels this as a game estimate, not physiology.

## Acceptance criteria

**AC1. Packaged runtime correctness.** The desktop build stages every Rust runtime binary from
the effective Cargo release directory and Electron packages only those staged files. A check
proves a non-default `CARGO_TARGET_DIR` artifact is the artifact packaged as `Kosmos Runtime.exe`;
the launcher calculator/currency E2E passes against the freshly built backend.

**AC2. Common integrations page.** The dashboard sidebar exposes `Интеграции` above `Тело`,
objects, and tracked time. Its Hevy and Toggl Track cards open the shared provider settings form
inside `@kosmos/visuals` `Modal`. Credential status/input, official credential link, interval
presets, startup sync, last-result status, provider-specific history range, and manual sync use
one shared settings shape; the old settings-tab entry is removed.

**AC3. Credential safety.** Credentials are stored in the OS keyring and are never returned to
the renderer after saving. Clear/replace and invalid-credential states work. The supplied Hevy
credential is configured only in the user's local keyring and validates against Hevy. A repository
and task-artifact scan contains no credential value.

**AC4. Hevy import.** Hevy sync reads official workout and exercise-template endpoints, annotates
workout exercises with primary/secondary muscle groups, upserts deterministic `workout_obj` ARK
objects, handles incremental workout update/delete events, and persists a safe cursor/status.
Provider/network failures retain prior imported data and return an actionable Russian status.

**AC5. Toggl Track import.** Toggl sync uses the official v9 API-token Basic Auth contract, imports
the configured time window into deterministic `time_entry_obj` ARK objects, preserves running
entries, handles deleted entries, and updates a safe cursor/status. Contract tests prove the
request/auth and mapping behavior without requiring a real user token.

**AC6. Scheduling.** A runtime scheduler honors per-provider interval and startup settings,
does not overlap manual/background imports, skips disconnected providers, and never blocks local
backend readiness. Interval validation prevents abusive or invalid polling values.

**AC7. Built-in body experience.** The shell dashboard has a directly launchable `Тело` section,
not an extension. `Развитие` colors front/back muscles by a 0–5 body-weight-relative game level
and explains the score; `Нагрузка` colors the same map by Hevy tonnage for week/month/year. Empty,
loading, disconnected, and missing-body-weight states are usable and Russian-language. Muscle
regions are keyboard accessible and expose Russian labels.

**AC8. Fresh proof.** Targeted Rust tests, desktop tests, `bun run ark:guard:writes`, desktop
typecheck/build, formatting/docs checks, the calculator E2E, and the integrations/body E2E all pass
against the final worktree. Evidence includes current command output and final screenshots. Any
failed/unknown criterion creates `problems.md`, receives the smallest safe fix, and is reverified.
