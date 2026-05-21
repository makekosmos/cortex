# Task: Arrancador full React -> Vue/Vapor migration

## Summary

Migrate the entire `apps/arrancador` renderer from React to Vue using the repo's established Vue/Vapor patterns, preserving current product functionality and renderer/main-process contracts.

## Acceptance Criteria

- AC1: React renderer boot/runtime is replaced with Vue app bootstrap and Vue routing.
- AC2: All renderer pages and shared components are ported off React.
- AC3: Global state/providers/hooks are ported to Vue composables/stores/plugins.
- AC4: React UI primitive wrappers are replaced with Vue equivalents.
- AC5: React-specific renderer dependencies and build config are removed or no longer required for runtime.
- AC6: Renderer typecheck/tests/build validation run successfully against the migrated app, or any remaining blocker is explicitly documented in verification artifacts.
