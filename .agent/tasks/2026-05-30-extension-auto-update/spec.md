# 2026-05-30 — Extension Auto Update

## Цель

Kepler/Kosmos должен сам обновлять уже установленные user extensions из marketplace catalog без подтверждений, открытия Settings или ручного клика «Обновить».

## Scope

- In scope: production-slot background update check на старте и периодически, download + sha256 verify + existing `installFromPath` flow, logs, command refresh after successful install.
- In scope: update только для installed user extensions, не для repo dev-source extensions.
- In scope: semver-based update decision; равная или более старая catalog version не должна ставиться.
- Out of scope: auto-install новых extensions, UI progress, rollout channels, rollback after bad release, signature model.

## Acceptance Criteria

**AC1.** На production startup marketplace periodic check автоматически находит newer catalog versions for already installed user extensions and installs them without user confirmation or Settings UI interaction.

**AC2.** Auto-update reuses the existing marketplace download + sha256 validation + `installFromPath` backup/atomic install flow; it does not duplicate extraction/backup logic.

**AC3.** Auto-update skips dev-source extensions, missing catalog entries, invalid semver, equal versions, and older catalog versions.

**AC4.** Successful background extension install notifies shell windows through the existing commands refresh path so launcher commands can update without app restart.

**AC5.** Failure to update one extension is logged and does not stop checks for the remaining extensions or crash startup.

**AC6.** Verification passes: focused pure tests for update planning, `bun run --cwd shell typecheck`, `bun run --cwd shell build:js`, `bun run docs:check`, and formatting checks for touched files.
