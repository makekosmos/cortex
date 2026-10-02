use std::env;
use std::path::PathBuf;

// KOS-306: ship every exe with icon + VERSIONINFO + application manifest —
// Defender's ML treats a bare exe with empty CompanyName/FileDescription as
// dropper-shaped. `pe-version-info` holds the single resource script shared
// with manager-gpui's build.rs.
// Build script: a failure must abort the build — panic/expect is the
// mechanism, an Err return would be silently ignored.
#[allow(clippy::panic, clippy::unwrap_used)]
fn main() {
    println!("cargo:rerun-if-env-changed=MUNDUS_PRODUCT_VERSION");
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    pe_version_info::embed(&pe_version_info::ExeInfo {
        display_name: "Mundus Engine",
        exe_name: "mundus-engine.exe",
        version: product_version(),
        icon: Some(manifest_dir.join("../desktop/build/icon.ico")),
    });
}

/// `MUNDUS_PRODUCT_VERSION` is always set by build-backend.mjs /
/// build-package-components.mjs for shipped builds; a local `cargo build`
/// without it falls back to the crate version so the VERSIONINFO still
/// parses.
#[allow(clippy::panic, clippy::unwrap_used)] // see main — build-time failure aborts the build
fn product_version() -> [u32; 3] {
    let raw =
        env::var("MUNDUS_PRODUCT_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_owned());
    let parts: Vec<u32> = raw
        .split('.')
        .map(|part| {
            part.parse()
                .unwrap_or_else(|_| panic!("MUNDUS_PRODUCT_VERSION must be X.Y.Z, got {raw:?}"))
        })
        .collect();
    <[u32; 3]>::try_from(parts.as_slice())
        .unwrap_or_else(|_| panic!("MUNDUS_PRODUCT_VERSION must be X.Y.Z, got {raw:?}"))
}
