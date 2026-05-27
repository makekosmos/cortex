# Problems - Akasha standalone installer packaging

## 2026-05-27 - `makensis` not on PATH

Initial `native:package` verification built the `.kext` but could not write the installer because `makensis` was not in PATH. The package script now resolves `MAKENSIS_PATH`, PATH, and the local electron-builder NSIS cache before failing with installation instructions.

Status: fixed and reverified. `bun run --cwd shell native:package akasha` produced `Akasha Setup 0.1.0.exe`.
