# Dashboard

Dashboard is the read-only ARK inspector and usage analytics app. It is allowed
to inspect a selected ARK SQLite database from Electron main, similar to a
database console, but it must not write to ARK tables.

Renderer code only talks to `window.dashboardApi`. ARK access belongs in
Electron main services, with `@kosmos/ark` analytics APIs preferred when they
cover the requested view.

## Checks

Use isolated smoke databases only:

```powershell
bun run smoke:seed -- --db-path .tmp\smoke-dashboard.db
bun run smoke:analytics -- --db-path .tmp\smoke-dashboard.db
bun run test:e2e:smoke
```
