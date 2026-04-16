# Problems

- `bun run lint` fails in the current environment because `oxlint` tries to load `apps/eden/ts/vite.config.ts` and Node reports `ERR_UNKNOWN_FILE_EXTENSION`.
- `bun run build` fails in the current environment because Vite externalize-deps hits sandbox `spawn EPERM`.
