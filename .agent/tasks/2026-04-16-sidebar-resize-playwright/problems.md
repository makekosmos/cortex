# Problems

## AC4 blocked by environment

- `node ...playwright/cli.js test -g "sidebar resize handle changes dashboard sidebar width"` fails before executing the test body with `spawn EPERM` in `WorkerHost.startRunner`.
- `node --experimental-strip-types scripts/runE2ESmoke.ts` also fails before renderer assertions with `electron.launch: spawn EPERM`.

These failures are caused by process-spawn restrictions in the current sandbox, not by the resize assertion itself.
