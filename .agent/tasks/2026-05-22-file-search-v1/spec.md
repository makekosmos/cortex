# File Search v1

## Goal

Add the first Kepler file search implementation: a non-elevated host-local file
index for local Windows drives that surfaces filename/path matches in the
launcher and gives the user control over the default noisy-folder exclusions.

## Context

- Kepler already has a host-local `app_index` in `kepler-backend` and merges
  indexed apps into launcher results.
- File Search v1 intentionally does not use NTFS MFT/USN fast scanning yet.
  This phase should establish the product contract and fallback indexer first.
- File index data is machine-specific and regenerable. It must not live in ARK
  or participate in ARK sync.

## Scope

### In

- Rust file index storage and filename/path search in `kepler-backend`.
- Initial indexing of files on local fixed drives with non-elevated filesystem
  traversal.
- Default noisy-folder exclusions with a setting that can enable/disable those
  exclusions and trigger a fresh scan.
- Launcher integration for file search results and opening selected files.
- Focused automated coverage for indexing, exclusions, and shell type/build
  contracts.

### Out

- Content search inside files.
- Folder results in the launcher.
- NTFS MFT/USN Fast File Scanner and privileged helper/service work.
- Network shares and removable drive search.
- ARK object storage or sync for file-index records.

## Acceptance Criteria

**AC1.** `kepler-backend` owns a host-local file index stored outside ARK and
can index/search files from local fixed-drive roots using the non-elevated v1
scanner.

**AC2.** The v1 scanner excludes noisy folders by default, and backend tests
prove that excluded folders do not enter search results while regular files do.

**AC3.** The Kepler launcher shows file results for non-empty filename/path
queries, distinguishes them from apps/commands, and invoking a file result opens
that file through the backend.

**AC4.** Settings expose a Russian-language File Search control for the default
noisy-folder exclusions; changing it persists per Kepler instance and triggers a
reindex with the new exclusion mode.

**AC5.** Verification is green for the touched backend and shell surfaces:
focused backend tests, `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`,
`bun run --cwd shell typecheck`, `bun run --cwd shell build:js`,
`bun run ark:guard:writes`, and `bun run ark:smoke`.
