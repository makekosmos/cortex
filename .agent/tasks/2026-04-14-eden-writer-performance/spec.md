# Task Spec — Eden writer-first performance + zen mode

## Source of truth

- `.omx/plans/prd-eden-performance.md`
- `.omx/plans/test-spec-eden-performance.md`
- `.omx/specs/deep-interview-eden-performance.md`

## Goal

Make Eden feel writer-first and highly native during typing, preserve TipTap, add a hotkey-driven zen mode, reduce renderer hot-path pressure, and add measurable perf instrumentation/regression coverage.

## Acceptance Criteria

- AC1: Normal-mode typing path avoids visible/perceived lag during ordinary writing.
- AC2: Zen mode is toggleable by hotkey and hides/suspends non-essential chrome/work rather than only styling it away.
- AC3: TipTap editor features continue to work without regression.
- AC4: Save reliability and note compatibility are preserved.
- AC5: Measurable typing/perf metrics are collectable for normal and zen mode.
- AC6: There is explicit regression coverage for save races and a composition/IME-oriented input path.
- AC7: Any sidecar work remains optional/conditional; no premature sidecar rewrite.

## Planned implementation scope for this Ralph session

1. Add instrumentation hooks for typing/update timing and expose summary for verification.
2. Reduce editor hot-path work by replacing snapshot-heavy dirty detection with revision/dirty tracking.
3. Wire a real zen mode hotkey + runtime policy that unmounts/suspends non-essential UI surfaces.
4. Add/adjust Playwright coverage for zen mode and typing/perf-related regression checks.
5. Verify with build, lint/typecheck, and e2e.

## Constraints

- TipTap stays.
- No GPUI.
- No note format changes.
- No editor/save regressions.
