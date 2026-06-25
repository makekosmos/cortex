# System Types Dead Export Cleanup

Goal: reduce `DEAD_EXPORT` high findings in `products/eden/src/lib/systemTypes.ts` without changing runtime behavior.

Change made:

- removed `export` from `SYSTEM_TYPE_GAME`
- removed `export` from `SYSTEM_TYPE_WORKOUT`
- removed `export` from `SYSTEM_TYPE_EXERCISE`

Blocked names:

- `SYSTEM_TYPE_WORKOUT_ID`
- `SYSTEM_TYPE_EXERCISE_ID`

Reason for blocker:

- both IDs are imported and used outside `systemTypes.ts` in `products/eden/src/components/spaces/SpacesView.vue`

Verification:

- targeted test passed
- platform desktop typecheck passed
- desloppify scan still reports two `DEAD_EXPORT` highs for the ID constants
