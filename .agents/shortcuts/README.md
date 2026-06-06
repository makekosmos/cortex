# Agent Shortcuts

Shortcuts are small reusable recipes for repeated Kosmos agent mistakes, command workarounds, and debugging paths that are useful but not yet worth a full skill.

Use this folder when the final skill audit finds something repeatable and the lesson does not clearly belong in an existing `.agents/skills/<name>/SKILL.md`.

## File Shape

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

## Rules

- Keep shortcuts short and operational.
- Prefer updating an existing skill when the shortcut already has a clear home.
- Do not store one-off task notes here; use `.agent/tasks/<date>-<slug>/` for task evidence.
- Do not run full docs sync/check just because a shortcut changed. Those run before commit/push or on explicit request.
