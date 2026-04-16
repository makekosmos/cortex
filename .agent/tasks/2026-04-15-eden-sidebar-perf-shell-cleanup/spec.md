# Task Spec — Eden sidebar perf shell cleanup

## Sources of truth
- `.omx/context/eden-sidebar-perf-shell-cleanup-20260415T142952Z.md`
- `.omx/plans/prd-eden-sidebar-perf-shell-cleanup.md`
- `.omx/plans/test-spec-eden-sidebar-perf-shell-cleanup.md`

## Cleanup plan
1. Remove/reduce heavy shell effects around the sidebar and content seam.
2. Replace full-array recent sidebar sorting with a cheaper derivation.
3. Remove legacy vault-sidebar runtime state from the active Eden UI path.
4. Reverify with lint/typecheck/build/e2e.

## Acceptance criteria
- AC1: Sidebar shell CSS is lighter.
- AC2: Recent sidebar derivation is cheaper than full sort on every recompute.
- AC3: Legacy vault-sidebar runtime state is removed from current UI flow.
- AC4: All verification commands pass.
