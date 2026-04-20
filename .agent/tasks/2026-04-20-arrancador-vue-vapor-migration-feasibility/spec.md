# Task: Arrancador Vue Vapor migration feasibility

## Summary
Assess whether `apps/arrancador` can be fully migrated from React to the latest Vue + Vapor stack in a safe, supportable way.

## Findings

- The current renderer is deeply coupled to React:
  - `src/main.tsx` mounts with `ReactDOM.createRoot`.
  - `src/router.tsx` uses `react-router-dom` lazy routing.
  - `src/providers.tsx`, `src/store/GamesContext.tsx`, `src/components/theme-provider.tsx`, `src/components/language-provider.tsx`, and `src/components/ToastProvider.tsx` are React context/provider based.
  - `src/components/ui/*` wraps multiple Radix React primitives.
  - The test stack is built around `@testing-library/react`.
- Package/tooling is React-specific:
  - `@vitejs/plugin-react`
  - `react`, `react-dom`, `react-router-dom`
  - `@radix-ui/react-*`
  - `@testing-library/react`
  - explicit React aliases/dedupe in `vite.config.ts`
- Official Vue Vapor status is still experimental:
  - `vuejs/vue-vapor` is archived and explicitly says active work moved to the core vapor branch.
  - The public Vue core Vapor roadmap still tracks unfinished ecosystem items including Vue Router, Pinia, and Vue Test Utils support.

## Conclusion

A full migration of `arrancador` to "latest Vue using Vapor" is not a safe one-pass refactor today. It would require:

1. Replacing the renderer framework and router.
2. Rewriting all route views and shared UI primitives.
3. Replacing React-only vendor dependencies.
4. Replacing the entire renderer test stack.
5. Depending on an experimental Vapor path whose ecosystem support is still incomplete.

## Safe next step

If migration is required, the defensible path is:

1. Migrate to stable Vue 3 + Vite + Vue Router first.
2. Keep the architecture Vapor-friendly:
   - Composition API
   - `<script setup lang="ts">`
   - explicit props/emits
   - composables for feature logic
   - thin route components
3. Evaluate Vapor later on a dedicated experimental branch after router/test support is mature enough for this app.
