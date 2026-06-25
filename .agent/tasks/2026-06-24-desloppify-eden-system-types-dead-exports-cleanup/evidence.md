# Evidence

`products/eden/src/lib/systemTypes.ts` now has 3 fewer `DEAD_EXPORT` highs:

- before: 5 dead exports, 342 findings, 185 high
- after: 2 dead exports, 339 findings, 182 high

The remaining dead exports are the ID constants:

- `SYSTEM_TYPE_WORKOUT_ID`
- `SYSTEM_TYPE_EXERCISE_ID`

Those stay exported because they are used in `products/eden/src/components/spaces/SpacesView.vue`.
