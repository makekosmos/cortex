# Task: Canonical ARK SDK and object-first migration groundwork

## Context

The ARK audit identified that `@arksync/node` started as a sync-focused package and that apps still refer to it as the public integration layer. The desired direction is a canonical `@kosmos/ark` SDK over `ark-core-rpc`, with the old package kept only as a compatibility bridge while app code moves to the new name.

The user also chose object-first migrations for Delphi tasks and Eden notes. This task handles the safe groundwork that can be implemented and verified locally first: canonical package naming, app import rewiring, documentation, and explicit TODO/manual items for the larger app data migrations.

## Acceptance Criteria

AC1. A new workspace package `@kosmos/ark` exists and exports the current ARK Node/Electron SDK API.

AC2. `@arksync/node` remains available as a compatibility package that re-exports `@kosmos/ark`, so existing external imports are not broken abruptly.

AC3. Electron app code that directly imports the SDK is rewired to `@kosmos/ark` where practical in this pass.

AC4. Workspace package manifests and TypeScript path aliases resolve `@kosmos/ark`.

AC5. Documentation names `@kosmos/ark` as the canonical SDK and describes `@arksync/node` as compatibility-only.

AC6. TODO/docs record the user decisions: Delphi tasks and Eden notes should migrate to object-first models; mobile smoke remains deferred; transport encryption is out of scope for this pass.

AC7. Verification artifacts include package typecheck/build checks that can run locally plus scoped diff checks.
