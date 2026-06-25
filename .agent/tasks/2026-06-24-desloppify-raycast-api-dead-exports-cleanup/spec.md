# Raycast API dead-export cleanup

Scope: `packages/raycast-api/src/components.ts`

Goal: remove dead `export` modifiers from local props interfaces only, while keeping runtime component exports unchanged.

Changed declarations:

- `ListProps`
- `ListItemProps`
- `DetailProps`
- `GridProps`
- `GridItemProps`
- `ActionProps`

Validation:

- `rtk bunx oxlint packages/raycast-api/src/components.ts`
- `rtk bunx tsc --noEmit --moduleResolution bundler --module esnext --target es2022 --lib es2022,dom packages/raycast-api/src/components.ts`
- `rtk proxy cmd /c "set PATH=%CD%\\.tmp\\bin;%PATH%&& bunx desloppify scan --json . > .tmp\\desloppify-after-raycast-api-dead-exports-cleanup.json"`
