# Desloppify TimeColumn Story Name Cleanup

## Classification

FULL_LOOP. This is a narrow Storybook cleanup inside the ongoing desloppify proof loop.

## Goal

Remove the `NUMERIC_SUFFIX` finding in `packages/visuals/components/TimeColumn.stories.ts` without changing the rendered story.

## Change

- Renamed Storybook export `MinutesStep5` to `MinutesFiveMinuteStep`.
- Kept component args, template, and visible Russian labels unchanged.

## Verification

- PASS: `rtk err bun run --cwd packages/visuals build-storybook`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-time-column-story-name-cleanup.json"`

`build-storybook` generated `packages/visuals/storybook-static`; it was removed before the final scan so generated output did not pollute the evidence.
