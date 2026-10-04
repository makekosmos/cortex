# Native macOS package workers

The macOS path runs installed native workers through the existing package
supervisor, bootstrap authentication, runtime grants, capability broker and
registry. A package does not acquire authority merely because its binary can
be executed.

## Local verification

Build the native fixture from the Cortex root:

```sh
cargo build -p engine --bin package-worker-fixture --features package-worker-fixture
MUNDUS_WORKER_FIXTURE="$PWD/target/debug/package-worker-fixture" \
  cargo nextest run -p engine-packages -E 'test(macos) | test(installed_worker)'
```

Build the real Apple Silicon packages from the sibling integrations checkout:

```sh
cd ../integrations
node scripts/build-packages.mjs --out out/macos-arm64 --sequence 2 \
  --target aarch64-apple-darwin
cd ../cortex
cargo run -p engine-packages --example macos-packages-smoke -- \
  ../integrations/out/macos-arm64/packages.json
```

The archive smoke command uses the real PackageStore installer, verifies the
pinned archive size/hash, then checks each locally built native worker's hello
and clean stop. It is a development probe, **not a production launch API**:
its direct subprocess path does not test supervisor ownership or grants.
The supervisor lifecycle tests exercise those separately with an installed
fixture. Neither check uses real provider credentials or performs a live sync.

## Platform boundaries

- macOS workers are Mach-O binaries. Existing package manifests use an `.exe`
  archive entry name for both Windows and macOS; that suffix does not make
  the macOS artifact a Windows binary.
- The installer sets owner-only executable permissions on validated worker
  entries. Archive-provided permissions do not grant execution to other files.
- Launch uses scrubbed environment variables and a separate process group.
  Stop and Drop clean up processes still belonging to that group.
- Process groups are **not equivalent to Windows Job Objects or a sandbox**.
  A deliberately hostile worker can escape a group using a new session. The
  macOS implementation must not be represented as enforcing Windows's
  single-process, CPU-rate or job-memory limits.
- Other unsupported operating systems remain fail-closed.

## Release boundary

Local package builds do not update the production catalog. The integration
release must include an artifact for every platform declared by each manifest,
with matching manifests and verified size/hash pins. Version the changed
packages before publishing. macOS arm64 preparation does not imply Intel Mac
support; do not declare x86_64 until its artifacts have been built and tested.
