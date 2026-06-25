# Vitest Browser Focused Spec

## Trigger

When running one Eden `tests/components/*.spec.ts` file through Vitest Browser
Mode.

## Symptom

`vitest run --browser tests/components/Foo.spec.ts` can fail during startup with
`browser.instances was set in the config, but the array is empty` because the
spec path is interpreted as the browser name and filters out `chromium`.

## Do This

Run the focused spec from `products/eden` with an explicit browser name:

```powershell
rtk err bunx vitest run --browser=chromium tests/components/EntryTitle.spec.ts
```

For the full browser component suite, the package script remains fine:

```powershell
rtk err bun run test:vue
```

## Avoid

Do not pass the spec path immediately after bare `--browser`.

## Promote To Skill When

This pattern appears across more products or needs broader Vitest Browser
debugging guidance.
