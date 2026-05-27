# Evidence - Akasha standalone installer packaging

## Acceptance criteria

- AC1: PASS. `bun run --cwd shell native:package akasha` ran `cargo build --release -p akasha`, wrote `shell/release/native/akasha/akasha-0.1.0.kext`, and wrote `shell/release/native/akasha/Akasha Setup 0.1.0.exe`.
- AC2: PASS. `.kext` packaging is shared through `shell/scripts/extension-package-utils.mjs`; `publish-extension.mjs` and `package-native-release.mjs` both use it.
- AC3: PASS. `shell/.tmp/native-package/akasha/installer.nsi` installs to `$LOCALAPPDATA\Programs\Akasha`, creates Start Menu/Desktop shortcuts, writes HKCU uninstall metadata, registers `.epub` as `Akasha.exe --open "%1"`, and only removes app files/shortcuts/association on uninstall.
- AC4: PASS. Standalone fallback data dir is `%APPDATA%\Akasha`; Kepler mode still wins through explicit `--kosmos-user-data-dir`.
- AC5: PASS. Akasha docs and native extension docs describe `.kext` vs standalone installer, `native:package`, artifact paths, and the Kosmos-aware / not Kosmos-required runtime model.

## Artifacts

- `shell/release/native/akasha/akasha-0.1.0.kext` - 17,516,659 bytes, SHA256 `C90D3A62EDC0D63ABF06AABFDD2566E7E99D9C01B702C834EFBF1DF324E07894`.
- `shell/release/native/akasha/Akasha Setup 0.1.0.exe` - 4,547,153 bytes, SHA256 `02A838E78CCFC77A742834BA0FA92713DBD9245A1926C926BFE77C3D110017A5`.

## Verification

- PASS `node --check shell\scripts\package-native-release.mjs`
- PASS `node --check shell\scripts\extension-package-utils.mjs`
- PASS `node --check shell\scripts\publish-extension.mjs`
- PASS `cargo fmt -p akasha`
- PASS `cargo test -p akasha`
- PASS `cargo check -p akasha`
- PASS `bun run --cwd shell typecheck`
- PASS `bun run ark:guard:writes`
- PASS `bun run --cwd shell native:package akasha`
- PASS `.kext` contents inspected: `manifest.json`, `icon.svg`, `README.md`, `bin/akasha.exe`.
- PASS `bun run docs:sync`
- PASS `bun run docs:check`
- PASS `bun run test:e2e -- tests/e2e/extensions-contract.spec.ts`
