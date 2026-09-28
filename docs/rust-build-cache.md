# Rust build and Core sidecar reuse

**Status: historical.** The `ark-core-rpc` sidecar and its cache machinery
(`desktop/scripts/ark-core-rpc.mjs`, `desktop/.tmp/ark-core-rpc`,
`KOSMOS_ARK_TARGET_DIR`, `ARK_CORE_RPC_PREBUILT`, `ARK_CORE_RPC_PATH`) were
removed when the Engine started hosting `ark-core` in-process. The sections
below are kept as the design/measurement record.

## Baseline

Measured on Windows 10.0.26200.0, x64, Rust/Cargo 1.95.0, Node 24.15.0,
and Bun 1.3.14 at Cortex `origin/main` `4c1f2a139fcae92ac0cf995b40ea63a7713c3b63`.
Cortex pins Core revision `169c1967a074ae6658e81d59892247b24332ce29`.

```text
rtk proxy cargo build --locked --bin ark-core-rpc --features iroh-spike --target-dir <dedicated> --timings
rtk proxy cargo build --locked -p engine --bin mundus-engine --features windows-gui-subsystem --target-dir <dedicated> --timings
```

| Build                                            |  Cold | Warm, unchanged | After a Rust edit |    Dedicated target |
| ------------------------------------------------ | ----: | --------------: | ----------------: | ------------------: |
| Core `ark-core-rpc`, `iroh-spike`                | 2m47s |           0.93s |            14.25s | 4,591,137,913 bytes |
| Cortex `mundus-engine`, `windows-gui-subsystem` | 3m29s |           1.01s |            14.39s | 5,322,661,606 bytes |

The six `cargo --timings` HTML reports were kept outside the repository in
`.tmp/kos14-evidence`; they are evidence, not release inputs. Dedicated
targets and temporary worktrees were removed after measurement.

## Sidecar cache

`desktop/scripts/ark-core-rpc.mjs` caches by pinned Core revision, debug/release
profile, and sorted feature names. A per-key lock is an exclusive Node file
creation. Waiting processes reuse the completed entry; dead-process or old
malformed locks are reclaimed. Cargo installs into a temporary sibling, writes
`.complete`, and the complete directory is renamed into place. A failed build
does not publish a partial entry.

The cache is at `cortex/desktop/.tmp/ark-core-rpc`. A cache miss is expected after a
Core revision, profile, feature, or cache-root change. For a deliberate reset,
stop active builds and remove only that directory; the next call reports Cargo
output and rebuilds it. Do not remove a lock while its recorded process is
alive.

## `MUNDUS_ARK_TARGET_DIR`

A source build sets `CARGO_TARGET_DIR` for `cargo install` so Cargo output
stays inside the staging directory instead of the global Cargo target. On
Windows the staging path under `desktop/.tmp/ark-core-rpc` grows past ~260
characters once platform, revision, profile, and feature keys join it, and
`ml64`/`link.exe` then fail with `MASM A1009: line too long` or `LNK1104`.
Set `MUNDUS_ARK_TARGET_DIR` to a short absolute path to redirect Cargo output:

```text
$env:MUNDUS_ARK_TARGET_DIR = 'C:\ark-target'
bun run --cwd desktop dev
```

When the variable is unset the target dir stays `<staging>/target` beside the
staged `--root`. Prebuilt manifests never invoke Cargo, so the override only
affects source builds.

## Explicit prebuilt mode for UI development

The default remains a source build. UI development may opt in explicitly:

```text
$env:ARK_CORE_RPC_PREBUILT = 'C:\path\to\ark-core-rpc-manifest.json'
bun run --cwd desktop dev
```

The manifest must sit beside the named binary and contain the current platform,
architecture, pinned Core revision, exact binary name, and SHA-256:

```json
{
  "platform": "win32",
  "arch": "x64",
  "coreRevision": "169c1967a074ae6658e81d59892247b24332ce29",
  "binary": "ark-core-rpc.exe",
  "sha256": "<64 lowercase hexadecimal characters>"
}
```

With no target directory, a valid prebuilt is copied into a cache keyed by its
platform, architecture, Core revision, and manifest hash; callers never receive
the mutable manifest source path. Invalid, missing, stale, or mismatched manifests are diagnosed as
`[ark-core-rpc] prebuilt rejected: ...` and fall back to the pinned source
build. An arbitrary `ARK_CORE_RPC_PATH` is not used by this mode. Release
builds do not consume the UI prebuilt setting.

First publication stages and verifies a uniquely named file, then atomically
renames it into the absent target. Replacement uses a uniquely named backup plus
rename and rollback; if a process dies after moving the old path to its backup,
the next invocation restores or reconciles that owned backup before use. A
failed publication leaves the prior target or cache entry recoverable.

## CI cache (retired)

Hosted GitHub Actions CI has been removed (the org does not provision paid
runner minutes). This section is retained as the design record for cache keys
in case local or future remote caching reuses them.

The Linux `portable` and Windows `windows-runtime` jobs cache Cargo git/db,
registry, `target` output, and the actual `desktop/.tmp/ark-core-rpc` sidecar
cache with the pinned `actions/cache` v4 action. Keys
include OS, runner architecture, a hash of exact `rustc -Vv` and `cargo -V` output, Cargo.lock,
Cargo manifests/configuration, profile, features, and the job's material build
scope. Dependency caches alone use an OS/toolchain restore prefix; target
output uses exact keys. Target output is an untrusted CI convenience cache and
the published sidecar is never cached as a trusted release artifact. GitHub's
PR cache scope cannot write into the base branch's cache.

`CARGO_BUILD_JOBS=1` remains test-only behavior in
`runtime/scripts/test-lib.mjs`; changing it was not measured. Peak-memory
tradeoffs are `NOT_RUN`.

Hosted CI evidence is `NOT_RUN` because KOS-50 billing/spending prevents runs;
this is not a code failure. `sccache` was not installed because the baseline
machine did not have it and no measurement justified adding another cache
layer.
