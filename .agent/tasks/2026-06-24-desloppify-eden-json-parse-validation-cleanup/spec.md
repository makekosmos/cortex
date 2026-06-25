# Desloppify Eden JSON Parse Validation Cleanup

## Classification

FULL_LOOP. This is a narrow Eden parsing cleanup inside the ongoing desloppify proof loop.

## Goal

Remove two `JSON_PARSE_CAST` findings in Eden content/title helpers without changing markdown or untitled-title behavior.

## Change

- Removed an unnecessary `as unknown` cast after `JSON.parse` in `readEntryMarkdown`.
- Added `isHeaderPropsRecord` before reading parsed untitled-title header props.
- Kept invalid JSON and non-object parsed values falling back to existing safe behavior.

## Verification

- PASS: `rtk err bun test products/eden/tests/content.test.ts`
- PASS: `rtk err bunx vitest run tests/components/CmConvert.spec.ts tests/components/EntryTitle.spec.ts --browser=chromium`
- PASS: `rtk err bun run --cwd platform/desktop build:extension eden`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-json-parse-validation-cleanup.json"`

The initial combined `bun test` command was the wrong runner for browser-mode specs and failed before executing them; the specs were rerun with Vitest Browser and passed.
