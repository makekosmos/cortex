# Problems Fixed During Verification

## P1: Coverage failed after adding wrapper components

- Symptom: `bun run test:coverage` failed because global function coverage dropped to 60.27%.
- Cause: new game-detail wrapper components were included by the existing coverage glob but only partially tested.
- Fix: added explicit composition tests for route shell helpers, section event forwarding, dialog event forwarding, and dangerous actions.
- Verification: final `bun run test:coverage` passed with 77.16% function coverage.

## P2: Lint import ordering failed

- Symptom: `bun run lint` failed on Biome import ordering.
- Fix: ran/check-applied Biome-compatible import ordering.
- Verification: final `bun run lint` passed.

## P3: E2E passed tests but timed out during cleanup

- Symptom: Playwright reported 4/4 passing tests, but `npm.cmd run test:e2e` did not exit before timeout.
- Cause: Vite preview process tree could remain alive on Windows after the test runner completed.
- Fix: updated `scripts/run-e2e.ts` to terminate the preview process tree with `taskkill /T /F` on Windows and wait for exit.
- Verification: final escalated `npm.cmd run test:e2e` passed and exited normally.
