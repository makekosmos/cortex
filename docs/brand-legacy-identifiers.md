# Brand legacy identifiers (KOS-266)

The product was renamed **Kosmos → Mundus** for 0.10.0. Most `kosmos`/`kepler`
strings were renamed; the identifiers below are deliberately kept because they
are persisted in user state, pinned external code, or remote services we do
not control. `scripts/check-brand.mjs` enforces this list — it reads
`scripts/brand-allowlist.json` and fails on any other `kosmos`/`kepler` hit.

## Persisted / remote identifiers (must NOT change in 0.10.0)

| Identifier                                                                 | Where                                                                                                                                                           | Why it stays                                                                                     |
| -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `makekosmos` (GitHub org/repo URLs)                                        | component pins, updater channel `makekosmos/cortex`, `imago` git deps (`imago-gpui`, `mundus-gpui-kit`), docs links | Org rename is a separate change; GitHub redirects keep old URLs valid.                           |
| `com.kosmos.*`                                                             | signed-catalog package ids                                                                                                                                    | Package ids remain part of the signed catalog contract. ARK canonical type ids moved to `core.*@0.1.0` in KOS-436. |
| `kosmos-root-2026`, `kosmos-release-2026`                                  | `package_service` signing key id                                                                                                                                | The key id is embedded in signed catalogs and installed trust state; rotation is KOS-265.        |
| `publisher: "kosmos"`                                                      | package manifests persisted by old installs                                                                                                                    | Persisted contract in already-installed package state.                                           |
| `extensions.kosmos` / `"kosmos":` JSON keys                                | ARK object extensions in user DBs (`taskBucket` etc.)                                                                                                           | Persisted user data.                                                                             |
| `application/vnd.kosmos.*`                                                 | media types in ARK objects                                                                                                                                      | Persisted user data.                                                                             |
| `@kosmos/ark`, `@kosmos/visuals`                                           | npm scope of published packages, WS `client_class`                                                                                                              | Published artifacts + wire values from existing clients.                                         |
| `x-kosmos-*` HTTP headers                                                  | Engine HTTP API + user_data handler                                                                                                                             | Pinned component builds and imago's `mundus-gpui-kit` send exactly these names.                          |
| `packages.kosmos.dev` / `*.kosmos.dev`                                     | catalog archive URLs, test fixtures                                                                                                                             | Hosted zone; follows the domain rename.                                                          |
| `kosmos-engine`                                                            | `engine-manifest.json` product field of installed 0.9.x Engines                                                                                                 | Installed manifests on user machines; the install script validates both ids.                     |
| `kosmos-link-v1`                                                           | ARK deterministic link-id hash seed (`ark-core`)                                                                                                                | Hash seed — changing it changes every derived link id in user DBs.                               |
| `kosmos-local://richtext-image/…`, `kosmos-local-image://file/…`           | ARK `richTextImages` keys and note/vault image URLs                                                                                                             | Written into persisted note objects; renaming orphans existing references.                       |
| `kosmos-icon://`                                                           | Electron-era icon protocol referenced by pinned component builds                                                                                                | Wire/display name already sent to and rendered by existing clients.                              |
| `extensions.kosmos` JSON key (incl. `/props/extensions/kosmos/…` pointers) | ARK object extensions (`taskBucket` etc.)                                                                                                                       | Persisted user data.                                                                             |
| `kosmos-host` / `TargetRuntime::KosmosHost`                                | `targets[].runtime` in package manifests                                                                                                                        | Signed manifests and installed package state use this value.                                     |
| `kosmos-desktop`                                                           | WS `client_class` sent by pinned components; usage-accounting keys                                                                                              | Pinned sibling builds report it; recorded usage keys already contain it.                         |
| `kosmos-kepler`                                                            | `brand::KEYRING_SERVICE` — Windows Credential Manager service for user API keys + integration secrets                                                           | Renaming orphans every stored credential.                                                        |
| `kepler-fallback`                                                          | fallback device id written into `*-device-id.txt`                                                                                                               | Existing installs persist this value in sync identity.                                           |
| `window.kosmosApp` / `window.kepler`                                       | legacy page globals read by pinned component builds                                                                                                            | Renaming breaks already-built clients; the Engine no longer serves pages.                         |

## Legacy data-dir move

The only remaining install-time migration is the Kosmos→Mundus data-dir move:
`%APPDATA%\Kosmos` and `%LOCALAPPDATA%\Kosmos` are renamed on first start so
existing user data is not orphaned.

| Identifier                                           | Where it lives                                       |
| ---------------------------------------------------- | ---------------------------------------------------- |
| `%APPDATA%\Kosmos`, `%LOCALAPPDATA%\Kosmos`          | roaming/local user data roots; Engine renames them.  |
| `kepler-singleton.lock.db`, `kepler-device-id.txt`, `kepler-shell-settings.json`, `engine.lock.json` under `%APPDATA%\Kosmos`      | files inside the legacy roaming dir / written by the Engine shim.                                                                                                           |
| `Kosmos-dev`, `Mundus-dev`                                                                                                       | dev-worktree directory names recognised by the dictation local-models discovery; contributor checkouts keep pre-rename dir names                                            |
| `kepler-backend.exe`, `Kosmos*.exe`, `Kosmos.exe`                                                                                  | process/image names of running 0.9.x builds; installer kill-list + Engine-binary detection.                                                                                 |
| `KOSMOS_*` / `KEPLER_*` env vars                                                                                                   | read as fallbacks where a user or dev may have set them (`brand::env` / `brand.mjs::env` read `MUNDUS_*` first); `KOSMOS_*_VERSION` is still sent to pinned sibling builds. |
| `# === kepler-focus BEGIN/END ===`, `hosts.kepler-backup`                                                                          | hosts-file section written by the previous focus helper; recognised and rewritten on next write.                                                                            |

## Historical records

Changelog/release notes and decision/hand-off logs keep old names verbatim —
they describe the past. The check allows `CHANGELOG*`/`HISTORY*`/
`RELEASE_NOTES*` paths and a small explicit file list in the allowlist.
