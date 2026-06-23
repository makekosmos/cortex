# 2026-06-23 Desloppify Cleanup

## Classification

FULL_LOOP.

## Goal

Reduce the current `desloppify` noise with small, production-safe changes:
split the worst desktop god files enough for the audit to stop flagging them as
critical, improve the noisiest e2e test file without weakening coverage, and
keep dead-code findings auditable instead of hiding real issues.

## Context

Baseline was captured before implementation in
`baseline/desloppify-full-findings.json`.

Baseline headline metrics:

- Score: 0
- Findings: 552 total
- Severity: 2 critical, 318 high, 161 medium, 71 low
- Biggest categories: dead-code 301, test-quality 81, defensive-programming 55,
  complexity 54

## Scope

In scope:

- `platform/desktop/electron/extension-host.ts`
- `platform/desktop/electron/main.ts`
- Focused extracted helpers under `platform/desktop/electron/`
- `tests/e2e/focus-widget-controls.spec.ts`
- Minimal audit/tooling config only if needed to keep generated or external-code
  noise out of the report
- Proof-loop evidence under `.agent/tasks/2026-06-23-desloppify-cleanup/`

Out of scope:

- macOS dictation backend implementation
- Product behavior changes unrelated to the refactors above
- Broad dead-code deletion across incubator/packages
- Version bump or release

## Acceptance Criteria

**AC1. Baseline and final metrics are reproducible.**
The task contains baseline and final `desloppify scan --json .` outputs, plus a
short metrics summary showing total findings and severity/category counts.

**AC2. Desktop god-file critical findings are removed.**
The final `desloppify` report has no `GOD_FILE` critical findings for
`platform/desktop/electron/extension-host.ts` or
`platform/desktop/electron/main.ts`.

**AC3. Refactors preserve desktop build behavior.**
The desktop backend/frontend build checks used by the dev workflow pass after
each substantial code iteration, or any unrelated local blocker is documented
with the exact failing command and error.

**AC4. Focus widget e2e test quality improves without deleting coverage.**
`tests/e2e/focus-widget-controls.spec.ts` keeps the same user flows covered, has
fewer `SLEEPY_TEST`/`WEAK_ASSERTION` findings in final `desloppify`, and its
targeted Playwright spec is run headlessly.

**AC5. The final state is independently reviewable.**
Evidence maps each AC to commands and verdicts. If a verifier or subagent finds
a blocking issue, the issue is fixed and rechecked before completion.
