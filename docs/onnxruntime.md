# ONNX Runtime (`ort` + `load-dynamic`)

The Parakeet dictation path (`transcribe-rs` feature `onnx` → `ort`
2.0.0-rc.12) is built with ort's **`load-dynamic`** feature (KOS-345).

## What it changes

- `load-dynamic` enables `ort-sys/disable-linking`: the `ort-sys` build script
  returns early — **no download, no link step** at build time. Builds work
  offline / without access to the ONNX Runtime CDN.
- ONNX Runtime is `dlopen`'d at runtime on first use, instead of being linked
  into `mundus-engine`.
- Cargo feature unification note: `transcribe-rs` depends on `ort` with
  default features and exposes no `load-dynamic` passthrough (checked
  0.3.11/0.3.12). The engine therefore declares
  `ort = { version = "2.0.0-rc.12", features = ["load-dynamic"] }` directly —
  same version, features unify. `ureq`/`native-tls`/`openssl-sys` remain in
  the lockfile as *build-deps of `ort-sys`* (gated on `download-binaries`,
  which is still on via the transcribe-rs edge); they are compiled but their
  download path is never executed. Removing them from the graph requires
  upstream support in transcribe-rs (an `ort-load-dynamic` passthrough or
  `default-features = false`).

## Runtime resolution

`engine` resolves the dylib lazily inside the parakeet transcription path
(`ensure_onnxruntime` in `runtime/src/dictation/local/backend.rs`), in order:

1. `ORT_DYLIB_PATH` env var (full path to the library).
2. `onnxruntime.dll` / `libonnxruntime.dylib` / `libonnxruntime.so` next to
   the running executable.
3. The plain library name via the OS loader search path.

Resolution uses `ort::init_from`, so a missing library produces a clear
`LocalError` instead of a panic inside `ort`.

## Shipping with the installer

`desktop/scripts/build-backend.mjs` stages `onnxruntime.dll` into the engine
payload when it is provisioned, in order:

1. `ONNXRUNTIME_DLL` env var pointing at the DLL, or
2. `desktop/vendor/onnxruntime/onnxruntime.dll` (gitignored).

`buildEnginePayload` (`desktop/scripts/engine-distribution.mjs`) treats the
DLL as an optional payload file: when staged, it is listed in
`engine-manifest.json` with size+sha256 like every other engine file, so the
`install` subcommand verifies it. When absent the payload is unchanged —
parakeet dictation then fails at transcription time with the
"onnxruntime dylib not found" error above; every other engine function is
unaffected.

## Dev machines

Nothing extra is needed to *build* — the build script no longer touches the
network. To *run* parakeet dictation locally, drop the ONNX Runtime library
for your platform next to `target/debug/mundus-engine` (or
`target/release/`) or export `ORT_DYLIB_PATH`.

## Smoke impact

- Local dictation smoke (parakeet engine) requires the dylib provisioned as
  above; previously it relied on `copy-dylibs` output next to the binary.
- Offline/CI builds no longer need the ONNX Runtime CDN at compile time.
