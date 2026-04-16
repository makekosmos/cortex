# AGENTS.md

Project: usage-tracker

Purpose
- Windows-first background executable that records foreground application usage and writes it directly into Ark DB.

Key files
- `src/main.rs` Active tracker runtime: polling loop, session reconciliation, Ark DB writes
- `src/windows_capture.rs` Win32 foreground window sampling and idle detection
- `installer/install.ps1` User-level installer with optional autostart registration
- `installer/uninstall.ps1` Removes autostart and installed files
- `scripts/build-installer.ps1` Builds the release installer bundle

Rules
- Tracker writes directly to Ark DB and MUST update `lan_sync.version_vector` after direct entity writes.
- Default DB path remains `%APPDATA%\\Kepler\\ark.db` unless overridden by env or CLI.
- The installer is user-level only; do not convert this flow into a Windows Service without an explicit product decision.
- `src/main.rs` is the active runtime path. Extra files in `src/` are scaffolding until explicitly wired in.

Packaging
- `bun run build:release` builds the release executable.
- `bun run package:installer` creates `dist/KeplerUsageTrackerInstaller/`.
- The installer bundle is the supported Windows distribution flow for now: release exe + install/uninstall scripts + manifest.
- The packaging script also emits `dist/KeplerUsageTrackerInstaller.zip` and `.cmd` launchers for install/uninstall so Explorer-based setup works without manually invoking PowerShell.
