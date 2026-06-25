# Desloppify Return Await False Positives

## Trigger

When `desloppify` reports `REDUNDANT_RETURN_AWAIT`.

## Symptom

The finding points at `return await somePromise()` inside a function that has
`try/catch` fallback behavior or a `finally` cleanup block.

## Do This

Check the control flow before editing:

- Keep `return await` inside `try/catch` when the `catch` must handle promise
  rejection and return a fallback.
- Keep `return await` inside `try/finally` when cleanup must happen after the
  awaited operation completes.
- Only remove `await` for a direct return with no catch/finally semantics.

## Avoid

Do not blindly replace `return await installFromPath(tmpFile)` with
`return installFromPath(tmpFile)` when a `finally` deletes `tmpFile`.
Do not remove `await` from IPC wrappers that need to log async rejections.

## Promote To Skill When

This becomes part of a broader static-analysis false-positive triage workflow.
