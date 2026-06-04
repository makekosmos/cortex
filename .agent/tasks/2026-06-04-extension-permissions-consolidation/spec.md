# 2026-06-04 — extension permissions + consolidation pass

## Цель

Закрыть проблемы из приложенного review так, чтобы конкретные баги были исправлены
и проверяемы, а архитектурные предложения не смешивались с небезопасным
попутным рефакторингом.

## Скоуп

- Runtime enforcement для extension permissions в main-process IPC.
- Regression coverage для deny-by-default user-installed extension'ов и trusted
  first-party extension'ов.
- Raw ARK local write identity hardening in `@kosmos/ark`.
- Headless/test visibility guards for existing Electron windows.
- Delphi desktop legacy spaces/local JSON/P2P runtime cleanup.
- Akasha EPUB parser guardrails for oversized/hostile archives.
- Документационный drift, который review указал как фактический: stale root refs,
  docs-check coverage, bump-policy conflict, generated docs freshness.
- Repo hygiene / reproducibility issues that are actionable in this pass:
  `.agent` runtime artifacts ignore policy, Cargo.lock policy for binaries, Bun
  version alignment when mismatch is real, committed Android native runtime
  documentation, and `@kosmos/ark` source-only package policy.

## Не в скоупе

- Большой refactor god files (`db.rs`, `extension-host.ts`, `ws_server.rs`,
  `Editor.vue`) без отдельного proof loop.
- Новые ARK sync/FTS/property-test milestones, если они не являются текущими
  failing guards in this worktree.
- Akasha EPUB streaming/storage redesign, console logging platform, public npm
  publishing redesign for `@kosmos/ark`.

## Acceptance Criteria

**AC1.** `shell/electron/extension-host.ts` enforces extension capabilities in
`kepler:extension:ark:request` before forwarding operations to ARK/backend.
User-installed extensions are denied by default unless their manifest declares
the required permission.

**AC2.** First-party extensions resolved from the repo dev tree or bundled
resources remain trusted and keep their current behavior without requiring all
bundled manifests to duplicate broad permission lists.

**AC3.** The capability model supports at least these permissions:
`objects.read`, `objects.write`, `objects.write:<type_id>`, `usage.read`,
`commands.register`, `commands.invoke`, `focus.control`, `arrancador.scan`,
`arrancador.launch`, `userData.read`, `userData.write`.

**AC4.** User-data IPC handlers enforce `userData.read` / `userData.write` for
user-installed extensions and continue to isolate files by sender extension id.

**AC5.** A regression test proves a user-installed extension without permissions
cannot call a write operation such as `upsert_object`, while a manifest with the
appropriate permission can perform the allowed operation.

**AC6.** Existing first-party extension contract smoke still passes, proving the
trusted path did not regress bundled/dev extensions.

**AC7.** Documentation source under `docs-site/` reflects the new enforced
permission model and the postmortem entry is completed with fix, regression
protection, and prevention.

**AC8.** Docs drift called out in the review is addressed or converted into a
stronger local guard: stale root documentation refs are either fixed or covered
by `bun run docs:check`; generated docs are synced when docs-site sources change.

**AC9.** Repo hygiene/reproducibility findings are fixed when safe in this
worktree: `.agent` runtime artifacts are ignored, Cargo.lock is not ignored for
this binary workspace, and Bun version mismatch between `packageManager` and CI
is resolved if still present.

**AC10.** Required guards pass against the final worktree:
`bun run ark:guard:writes`, `bun run docs:check`, relevant shell typecheck/build
or targeted Playwright specs, and `bun run ark:smoke` unless a documented
environment blocker prevents it.
