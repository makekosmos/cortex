# Storybook Build Generated Scan Noise

## Trigger

When using `build-storybook` as verification before a repository-wide static scan such as `desloppify`.

## Symptom

The scan suddenly reports thousands of extra findings or a critical finding after a Storybook build.

## Do This

Run the Storybook build for verification, then remove the generated output before scanning:

```powershell
rtk err bun run --cwd packages/visuals build-storybook
rtk proxy powershell -NoProfile -Command "`$target = Resolve-Path -LiteralPath packages\visuals\storybook-static -ErrorAction SilentlyContinue; if (`$target -and `$target.Path.StartsWith((Resolve-Path -LiteralPath .).Path + '\packages\visuals\storybook-static')) { Remove-Item -LiteralPath `$target.Path -Recurse -Force }"
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after.json"
```

## Avoid

- Do not use the first post-Storybook scan as evidence if `packages/visuals/storybook-static` still exists.
- Do not remove broader package directories; only delete the resolved `storybook-static` output.

## Promote To Skill When

This pattern recurs across multiple generated-output tools or needs generalized cleanup rules for scan evidence.
