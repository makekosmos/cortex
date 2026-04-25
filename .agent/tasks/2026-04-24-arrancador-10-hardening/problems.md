# Problems

## Fixed In This Pass

1. Electron main/script TypeScript was not covered by normal typecheck.
   - Fixed by adding `tsconfig.electron.json`, making `tsconfig.node.json`
     self-contained, and running both from `bun run typecheck`.

2. Vue template prop contracts were not checked.
   - Fixed by adding `vue-tsc --noEmit` to `bun run typecheck` and correcting
     `GameDetailPage` -> `GameDetailDialogs` prop names.

3. `games.ts` had unresolved `getRowById` / `queryOne` hazards.
   - Fixed by importing `queryOne` and routing `recordGameLaunch` through the
     game loader path.

4. Backup copy paths allowed unsafe segments.
   - Fixed in TypeScript and Rust by rejecting traversal, absolute paths,
     drive-qualified paths, unsafe segments, and unsafe root labels.

5. Backup restore manifest `backupPath` validation accepted unsafe forms.
   - Fixed in TypeScript and Rust by normalizing before splitting and rejecting
     traversal, absolute paths, drive-qualified paths, and empty segments.

6. Backup restore trusted arbitrary manifest `originalPath` destinations.
   - Fixed by requiring trusted `allowedRestoreRoots` in TypeScript and Rust,
     deriving them from the current game/save-root context in the backup
     workflow, and rejecting out-of-root targets.

7. Managed sidecar stdout could race process close.
   - Fixed by serializing stdout drains and waiting for `stdoutDrain` before
     close-time rejection.

8. Managed sidecar stdin write errors could surface as unhandled errors.
   - Fixed by capturing stdin stream/write errors and routing them through the
     managed failure path.

9. E2E fallback retried inline scenarios for every Playwright failure.
   - Fixed by failing closed unless `ARRANCADOR_E2E_INLINE_FALLBACK=1`.

10. E2E sandbox cleanup produced a secondary `taskkill` EPERM stack.
    - Fixed by wrapping Windows `taskkill` cleanup in a real `try/catch`.

11. Coverage scope omitted active facades and nested implementation files.
    - Fixed by including backup IPC handlers, backup service facade,
      backup workflow nested modules, backup copy/restore, games facade, and
      sidecar wrapper; copied current lcov to `raw/coverage-lcov.info`.

12. Proof artifacts drifted from the hardening spec.
    - Fixed by rewriting `spec.md`, `evidence.md`, `evidence.json`,
      `changed-files.md`, and this file against the current code and current
      raw verification results.

13. `shell_open_external` accepted arbitrary URL schemes.
    - Fixed by validating `http:`/`https:` in both the preload/shared IPC
      bridge and the main-process handler, with tests covering blocked
      `file:`, `javascript:`, and custom protocol URLs.

14. The Electron renderer window was created with `sandbox: false`.
    - Fixed by enabling renderer sandboxing and adding a window-options test
      for `contextIsolation: true`, `nodeIntegration: false`, and
      `sandbox: true`.

15. The Windows sidecar launcher used `shell: true`.
    - Fixed by direct-spawning the sidecar with `shell: false` and testing that
      paths with spaces are passed as the executable path, not shell text.

16. Rust restore-root comparison mixed `/` and `\` path forms.
    - Fixed by normalizing separators before root comparison and adding a
      regression test for forward-slash allowed roots.

17. BrowserWindow `setWindowOpenHandler` used a weaker regex URL allowlist than
    the IPC external-open handler.
    - Fixed by parsing URLs with `new URL`, allowing only `http:`/`https:`,
      passing the normalized URL string to `shell.openExternal`, and testing
      blocked `file:` and `javascript:` URLs.

18. Rust restore manifest path validation accepted empty segments.
    - Fixed by rejecting empty `backup_path` segments instead of filtering them
      out, with regression coverage for repeated and trailing separators.

19. Sidecar stdin error handling was implemented but not directly tested.
    - Fixed by adding protocol tests for both stdin stream errors and write
      callback errors.

20. The changed-file scope artifact was not closed under imports for the
    changed Vue pages.
    - Fixed by moving the related game-detail/library components and tests into
      the task-owned scope so the review/staging guide is self-contained.

21. Raw proof artifacts contained stale summaries from earlier review passes.
    - Fixed by marking the `.txt` logs and `coverage-lcov.info` as canonical in
      `raw/current-run-manifest.md`, updating
      `raw/independent-review-commands.txt` to the current 45-file / 152-test
      run, and explicitly superseding the older `.log` files.

22. The proof artifact scope omitted `independent-review.md/json`.
    - Fixed by listing both files in `changed-files.md`.

23. Individual reviewers could interpret `AC9 PENDING` as a blocker for their
    own review.
    - Fixed by clarifying in `independent-review.md/json` that pending AC9 is a
      coordination state; reviewers should judge whether their review can count
      as one of the five required 10/10 reviews.

## Open

AC9 remains open until five fresh independent post-fix reviews return 10/10 in
architecture, readability, atomicity, and testing.
