# Problems

No unresolved problems.

## Resolved During Verification

- Arrancador usage backfill wrote completion/device markers directly to ARK `sync_kv`; fixed by adding `ArkClient.kv` and using `get_sync_kv` / `set_sync_kv` through `@kosmos/ark`.
- Eden custom typed note entries could remain Heart-backed after migration; fixed by saving all valid typed entries as ARK objects.
- Eden app tests only set `HOME`; fixed by adding `KOSMOS_TEST_APPDATA` / `KOSMOS_TEST_USER_DATA` support and wiring those env vars in Eden Playwright launchers.
- Eden built main process used named ESM imports from Electron; fixed by switching main-process Electron imports to default import destructuring.
- Eden typed-note menu did not expose submenu items for the existing e2e selector; fixed by opening the type picker with the menu and sharing the menu item class.
- ARK object-to-entry mapping always returned `header_layout: "default"`; fixed by deriving layout from ARK object type metadata.
- Dashboard smoke analytics initially failed in sandbox with `spawn EPERM`; rerun with approval passed against the task-local smoke DB.
