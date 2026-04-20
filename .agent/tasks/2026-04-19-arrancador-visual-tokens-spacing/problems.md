# Verification problem

- `bun run test` failed in `src/test/games-context.test.tsx`.
- Failure mode: the test asserted `latestContext?.getGame(...)` synchronously after the DOM reached `"ready"`, but the harness updates `latestContext` from a `useEffect`, so the assertion can observe a stale snapshot even when the provider state is already loaded.
- Smallest safe fix: wait for the context snapshot before asserting on `getGame`, without changing Arrancador runtime behavior.
