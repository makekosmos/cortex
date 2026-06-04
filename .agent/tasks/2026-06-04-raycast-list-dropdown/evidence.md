# Evidence - Raycast List.Dropdown host slice

Verified at: 2026-06-04T19:25:00+03:00

## Summary

PASS. The shell Raycast List host now renders a first `List.Dropdown` search-bar
accessory, including grouped dropdown options and trusted `onChange` callbacks
through the existing guarded session action IPC.

## Acceptance Criteria

- PASS: `ListProps.searchBarAccessory` added to `@raycast/api` shim types.
- PASS: normalizer moves `searchBarAccessory` into the serializable snapshot.
- PASS: `onChange` callbacks are registered as `__callbackId`.
- PASS: host model extracts dropdown sections, items, values, titles, and
  default value.
- PASS: Vue List host renders search input and dropdown side by side and invokes
  callback IPC on change.
- PASS: docs updated in `docs-site/concepts/extension-host.md` and
  `docs-site/apps/kepler-roadmap.md`.

## Automated Checks

- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-view-model.test.ts tests/unit/raycast-command-runner.test.ts`
  - 13 tests passed, 0 failed, 60 expectations.
- PASS: `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - 24 tests passed, 0 failed, 83 expectations.
- PASS: `bun run shell:typecheck`
- PASS: `bun run docs:sync`
- PASS: `bun run docs:check`
- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
  - First attempt hit a Windows file lock on `target/debug/ark-core-rpc.exe`
    from leftover workspace dev/test processes; after stopping only the confirmed
    workspace-target PIDs and the temporary Vite child, rerun passed.
  - ARK smoke matrix passed.

## Visual Verification

- PASS: local Vite renderer with mocked Raycast snapshot and session action IPC.
- Screenshot:
  `.tmp/visual/2026-06-04-raycast-host/raycast-list-dropdown-1000x720.png`
- Observed state: title `Поиск заметок`, search input, dropdown selected as
  `Заметки`, section `Недавние`, selected item `Сегодня`, detail panel, action
  button `Скопировать`, status `Выбрано`, no visible text clipping or overlap at
  1000x720.

## Remaining Scope

Async search throttling, controlled dropdown value reconciliation, and keyboard
shortcut parity remain for later Raycast UI phases.
