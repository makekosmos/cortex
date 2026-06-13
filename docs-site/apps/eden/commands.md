# Eden commands quick reference

Scope: `products/eden/manifest.json`, `src/main.ts`, Eden command/deep-link routing.

- Commands are declared in `manifest.json::commands[]`; launcher discovers extension commands from manifests.
- Eden routing converts shell routes/deep links into local command dispatch (`dispatchEdenCommand(...)`).
- Do not use `vue-router` for titlebar history; Eden keeps local history/navigation state.
- Register commands defensively in kepler-managed mode; standalone/self-managed startup should not crash if command APIs are absent.

Related: `docs-site/concepts/command-bus.md`, `docs-site/concepts/extension-host.md`, `docs-site/agents/forbidden/shell.md`.
