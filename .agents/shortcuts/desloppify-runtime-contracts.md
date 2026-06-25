# Desloppify Runtime Contracts

## Trigger

When a `desloppify` cleanup touches runtime, dictation, native IPC, packaging,
or performance-sensitive paths.

## Symptom

The cleanup looks smaller, but product behavior regresses: local STT falls back
from warmed sidecar/Vulkan to CPU CLI, a native transparent window returns to an
old compositing path, or a settings flow collapses a separate screen into an
inline block.

## Do This

1. Identify the product contract before deleting code: latency target, native
   backend, window behavior, persistence guarantee, or UX surface.
2. Search history and current tests for the contract names, not just the files:
   `sidecar`, `vulkan`, `preload`, `transparent`, `pending`, `queue`.
3. Keep the smallest implementation that preserves the contract. Delete only
   duplicate wiring, dead branches, or unused fallback layers.
4. Verify with a contract-level check, for example local STT sidecar tests,
   `cargo check`, and a screenshot for UI/window changes.

## Avoid

- Treating "complex" as "slop" when the complexity is a native/runtime boundary.
- Replacing a warmed GPU/sidecar path with CPU CLI just because the latter is
  shorter.
- Restoring or deleting whole historical files without reapplying current config
  fields and tests.

## Promote To Skill When

This becomes a regular review step for whole-repo cleanup runs, not only
dictation/runtime regressions.
