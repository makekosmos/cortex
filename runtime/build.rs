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
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    pe_version_info::embed(&pe_version_info::ExeInfo {
        description: "Mundus Engine",
        exe_name: "mundus-engine.exe",
        version: pe_version_info::product_version(&manifest_dir),
        icon: Some(manifest_dir.join("../desktop/build/icon.ico")),
        // The Engine has no manifest from any dependency — the helper owns it.
        manifest: true,
    });
}
