use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let versions_path = manifest_dir.join("../desktop/release-versions.json");
    let icon_path = manifest_dir.join("../desktop/build/icon.ico");
    println!("cargo:rerun-if-changed={}", versions_path.display());
    println!("cargo:rerun-if-changed={}", icon_path.display());
    // Icon + VERSIONINFO + application manifest stamped with the win release
    // version from desktop/release-versions.json — the packaged exe reports
    // the same version the installer and latest.yml carry. The resource
    // script itself is shared with runtime/build.rs via pe-version-info
    // (KOS-306); embedding is target-gated inside the helper, so
    // cross-compiles from non-Windows hosts still embed.
    pe_version_info::embed(&pe_version_info::ExeInfo {
        display_name: "Mundus Manager",
        exe_name: "Mundus Manager.exe",
        version: release_version(&versions_path),
        icon: Some(icon_path),
    });
}

/// Read the `win` release version from desktop/release-versions.json without
/// pulling a JSON parser into the build graph.
fn release_version(path: &PathBuf) -> [u32; 3] {
    let raw = std::fs::read_to_string(path).expect("read desktop/release-versions.json");
    let win = raw
        .split("\"win\"")
        .nth(1)
        .and_then(|rest| rest.split('"').nth(1))
        .expect("release-versions.json must contain a win version");
    let mut parts = win.split('.');
    let mut version = [0u32; 3];
    for slot in &mut version {
        *slot = parts
            .next()
            .and_then(|part| part.parse().ok())
            .expect("win release version must be X.Y.Z");
    }
    assert!(parts.next().is_none(), "win release version must be X.Y.Z");
    version
}
