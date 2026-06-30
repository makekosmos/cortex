# Eden Extension Dist Stale UI

## Trigger

When Eden UI source or Vite dev preview shows the expected behavior, but the desktop shell still shows an older visual state.

## Symptom

The user screenshot differs from the verified source build, especially for CSS/editor changes in `products/eden/src`, and `products/eden/dist/assets` does not contain the new class, selector, or compiled component code.

## Do This

1. Confirm the source has the change with `rtk rg`.
2. Confirm the built extension has it with `rtk rg "<class-or-symbol>" products/eden/dist/assets`.
3. Rebuild the Eden extension with `rtk bun run --cwd platform/desktop build:extension eden`.
4. Re-check `products/eden/dist/assets` and run the relevant visual check against `vite preview` or the desktop target.
5. Ask for a window/extension reload only after the rebuilt `dist` is confirmed current.

## Avoid

Do not keep patching source-only CSS when the desktop shell is loading a stale extension bundle.

## Promote To Skill When

This pattern repeats across more extension UI regressions or needs a standardized desktop-shell reload recipe.
