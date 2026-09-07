# AC1 Windows headless evidence

- Repository: `makekosmos/cortex`
- Worktree: `C:\Users\kirill\Coding\makekosmos\cortex`
- Tested HEAD: `f4d972cd176f03338fe98ea77c4603db63e1594e`
- Canonical `origin/main`: `20fd15a90c3d429dd0f9fef025583f2992ed4862`
- Command: `bun run --cwd host e2e first-party-agenda-contract.spec.ts --workers=1`
- Exact test: `signed Agenda installs, runs in Host, and survives Engine restart`
- Result: `PASS 1/1 (3.1m)`
- Observed: trust/catalog/install/set_enabled PASS; typed ARK writes before restart; Engine restart; package list and Host restart PASS.
- Scope: Windows headless only; no Core changes; GUI/Linux/macOS deferred by user.
