---
name: final-skill-audit
description: Before the final answer for Kosmos coding/debugging tasks, check whether a repeated failure mode, workaround, command recipe, or debugging path should be captured as a skill or shortcut. Use after bugs, failed commands, Windows-specific workarounds, build/release issues, or repeated UI verification traps.
---

# Final Skill Audit

Use this skill before the final answer for any coding/debugging task in Kosmos, especially after a bug, failed command, Windows-specific workaround, build/release issue, or repeated UI verification trap.

## Goal

Catch repeatable lessons while they are fresh, so the next agent does not rediscover the same fix.

## Hook

Before the final response, ask yourself:

> Did I find a repeated failure mode, workaround, command recipe, or debugging path that should become a skill or shortcut?

This is an internal check. Do not add noisy text to the final answer unless something was captured or there is an explicit follow-up.

## What to Capture

Capture when at least one is true:

- A command failed in a Windows-specific or sandbox-specific way and the fix is reusable.
- A build/release/test step required a non-obvious workaround.
- A bug root cause maps to a class of future mistakes.
- You had to remember a project convention that is easy to violate.
- The same diagnostic path would save time next time.

Do not capture one-off facts, temporary state, or obvious repo knowledge already covered by `AGENTS.md`.

## Destination

Choose the smallest durable place:

1. Existing skill: update it when the lesson clearly belongs there, e.g. `windows-sandbox`, `bump`, `update-docs`, `visual-verify`, `bug-postmortem`.
2. `.agents/shortcuts/<slug>.md`: use for a compact recipe that is useful but not yet broad enough to be a full skill.
3. New skill: create only when the pattern is broad, repeatable, and has a clear trigger.

For code bugs, still use `bug-postmortem`; this audit is not a replacement for postmortems.

## Shortcut Format

Use this shape for `.agents/shortcuts/<slug>.md`:

```markdown
# Short Title

## Trigger

When to use this shortcut.

## Symptom

What the agent/user sees.

## Do This

Concrete steps or commands.

## Avoid

Failed approaches or traps.

## Promote To Skill When

When this deserves a real `.agents/skills/<name>/SKILL.md`.
```

## Timing

- Update shortcuts/skills immediately if the edit is small and directly related to the task.
- Do not run full docs sync/check just because this audit updated agent docs. Run those only before commit/push or when explicitly requested.
- In the final answer, mention the capture briefly only when a shortcut/skill was actually changed.
