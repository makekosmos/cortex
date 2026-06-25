# Raycast host model dead-export cleanup

Goal: remove safe `DEAD_EXPORT` findings from `platform/desktop/src/raycast-host/model.ts` by dropping export modifiers only.

Kept exported: `gridItems`, because `tests/unit/raycast-view-model.test.ts` references it.
