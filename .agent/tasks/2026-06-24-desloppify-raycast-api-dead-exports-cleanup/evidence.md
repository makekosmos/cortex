# Evidence

The public package entrypoint does not re-export the props interfaces, and the grep scan found no external usage outside `packages/raycast-api/src/components.ts`.

Before:

- scan score: 9
- findings: 379
- high: 222
- medium: 113
- low: 44
- `components.ts` DEAD_EXPORT entries: 6

After:

- scan score: 9
- findings: 373
- high: 216
- medium: 113
- low: 44
- `components.ts` DEAD_EXPORT entries: 0

Checks:

- `rtk bunx oxlint packages/raycast-api/src/components.ts`
- `rtk bunx tsc --noEmit --moduleResolution bundler --module esnext --target es2022 --lib es2022,dom packages/raycast-api/src/components.ts`
- `rtk proxy cmd /c "set PATH=%CD%\\.tmp\\bin;%PATH%&& bunx desloppify scan --json . > .tmp\\desloppify-after-raycast-api-dead-exports-cleanup.json"`
