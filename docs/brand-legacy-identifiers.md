# Brand legacy identifiers (KOS-266)

The product was renamed **Kosmos → Mundus** for 0.10.0. Most `kosmos`/`kepler`
strings were renamed; the identifiers below are deliberately kept because they
are persisted in user state, pinned external code, or remote services we do
not control. `scripts/check-brand.mjs` enforces this list — it reads
`scripts/brand-allowlist.json` and fails on any other `kosmos`/`kepler` hit.

## Persisted / remote identifiers (must NOT change in 0.10.0)

| Identifier | Where | Why it stays |
| --- | --- | --- |
| `makekosmos` (GitHub org/repo URLs) | component pins, updater channel `makekosmos/desktop`, `imago`/`kosmos-gpui-kit` git deps, docs links | Org rename is a separate change; GitHub redirects keep old URLs valid. |
| `com.kosmos.*` | signed-catalog package ids, ARK canonical type ids (`com.kosmos.note/task/game`) inside user `ark.db` | Rewriting user objects risks sync corruption; KOS-265 owns the id migration. |
| `kosmos-root-2026`, `kosmos-release-2026` | `package_service` signing key id | The key id is embedded in signed catalogs and installed trust state; rotation is KOS-265. |
| `publisher: "kosmos"` | package manifest schema + signed manifests | Persisted contract — `package_manifest.rs` validates it. |
| `extensions.kosmos` / `"kosmos":` JSON keys | ARK object extensions in user DBs (`taskBucket` etc.) | Persisted user data. |
| `application/vnd.kosmos.*` | media types in ARK objects | Persisted user data. |
| `@kosmos/ark`, `@kosmos/visuals` | npm scope of published packages, WS `client_class` | Published artifacts + wire values from existing clients. |
| `x-kosmos-*` HTTP headers | Engine HTTP API + user_data handler | Pinned component builds and `kosmos-gpui-kit` send exactly these names. |
| `kosmos-gpui-kit` / `kosmos_gpui_kit` | `manager-gpui` pinned git dependency | External repo; renamed upstream separately. |
| `packages.kosmos.dev` / `*.kosmos.dev` | catalog archive URLs, test fixtures | Hosted zone; follows the domain rename. |
| `kosmos-engine` | `engine-manifest.json` product field of installed 0.9.x Engines | Installed manifests on user machines; the install script validates both ids. |
| `kosmos-link-v1` | ARK deterministic link-id hash seed (`ark-core`) | Hash seed — changing it changes every derived link id in user DBs. |
| `kosmos-local://richtext-image/…`, `kosmos-local-image://file/…` | ARK `richTextImages` keys and note/vault image URLs | Written into persisted note objects; renaming orphans existing references. |
| `kosmos-icon://` | Electron-era icon protocol referenced by pinned component builds | Wire/display name already sent to and rendered by existing clients. |
| `extensions.kosmos` JSON key (incl. `/props/extensions/kosmos/…` pointers) | ARK object extensions (`taskBucket` etc.) | Persisted user data. |
| `kosmos-host` / `TargetRuntime::KosmosHost` | `targets[].runtime` in package manifests | Signed manifests and installed package state use this value. |
| `kosmos-desktop` | WS `client_class` sent by pinned components; usage-accounting keys | Pinned sibling builds report it; recorded usage keys already contain it. |
| `kosmos-kepler` | Windows Credential Manager service for user API keys + integration secrets | Renaming orphans every stored credential. |
| `kepler-fallback` | fallback device id written into `*-device-id.txt` | Existing installs persist this value in sync identity. |

## Legacy install state (consumed by the KOS-267 migration)

These names exist on machines that ran 0.9.x/Electron-era installs. The
migration code paths that read or remove them are marked
`MIGRATION(KOS-267): remove after 2026-11-01`.

| Identifier | Where it lives |
| --- | --- |
| `%APPDATA%\Kosmos`, `%LOCALAPPDATA%\Kosmos` | roaming/local user data roots; Engine renames them on first start. |
| `%LOCALAPPDATA%\Programs\kepler-shell`, `...\Kosmos` | Electron-era program files; installer removes them. |
| `Uninstall\Kosmos`, `Uninstall\KosmosEngine`, bare GUID keys `4fe2b964…`, `af85bd72…` | old Apps & Features registrations; installer deletes unconditionally. |
| `Kosmos Engine`, `Kosmos`, `com.kazui.kosmos`, `electron.app.Kosmos`, `com.kazui.kepler`, `Kepler`, `KeplerKosmos`, `KosmosKepler` | HKCU Run values from every previous generation; read for the autostart decision, then deleted. |
| `Kosmos*.lnk`, `Kepler.lnk`, `CosCast.lnk` | old Start Menu/Desktop shortcuts; installer removes. |
| `KosmosSystemSvc`, `KeplerFocusSvc` | Windows service registrations of previous privileged services; the new `focus-svc` still answers to them and removes them on elevated `install`. |
| `\\.\pipe\kepler-focus-svc`, `\\.\pipe\kosmos-system-service` | named pipes of installed legacy services; Engine falls back to them. |
| `kepler-singleton.lock.db`, `kepler-device-id.txt`, `kepler-shell-settings.json`, `engine.lock.json` under `%APPDATA%\Kosmos` | files inside the legacy roaming dir / written by the Engine shim. |
| `kepler-backend.exe`, `Kosmos*.exe`, `Kosmos.exe` | process/image names of running 0.9.x builds; installer kill-list + Engine-binary detection. |
| `KOSMOS_*` / `KEPLER_*` env vars | read as fallbacks where a user or dev may have set them (`brand::env` / `brand.mjs::env` read `MUNDUS_*` first); `KOSMOS_*_VERSION` is still sent to pinned sibling builds. |
| `# === kepler-focus BEGIN/END ===`, `hosts.kepler-backup` | hosts-file section written by the previous focus helper; recognised and rewritten on next write. |

## Historical records

Changelog/release notes and decision/hand-off logs keep old names verbatim —
they describe the past. The check allows `CHANGELOG*`/`HISTORY*`/
`RELEASE_NOTES*` paths and a small explicit file list in the allowlist.
