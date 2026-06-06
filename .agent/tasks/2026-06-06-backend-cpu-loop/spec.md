# 2026-06-06 — Backend CPU loop

## Goal

Kosmos dev instance in idle burns sustained CPU in `kepler-backend.exe` and `ark-core-rpc.exe`.
Find the concrete source, fix it, and add regression protection so the same class of background loop cannot return unnoticed.

## Scope

- In scope: `services/kepler-backend` and ARK-side request patterns triggered by backend background tasks.
- In scope: tests and postmortem documentation for the discovered root cause.
- Out of scope: Codex Desktop performance, release/version bump, broad architecture rewrite.

## Acceptance Criteria

**AC1.** The root cause is identified to concrete code paths and supported by runtime evidence from the high-CPU session.

**AC2.** The fix prevents sustained idle CPU churn by making the offending background path bounded, event-driven, debounced, idempotent, or otherwise rate-limited as appropriate to the root cause.

**AC3.** A regression test fails on the old behavior or directly asserts the invariant that prevents the CPU loop from returning.

**AC4.** Data/write-boundary safety remains intact: any touched ARK writes still go through allowed Rust helpers and required guards pass.

**AC5.** The bug is recorded in `docs-site/agents/postmortems.md` with symptoms, location, root cause, fix, regression protection, and prevention.

**AC6.** A fresh post-fix 30-second CPU/RAM measurement shows `kepler-backend.exe` and `ark-core-rpc.exe` no longer burn sustained idle CPU comparable to the pre-fix evidence, or any remaining load is explained by a non-idle operation visible in logs.
