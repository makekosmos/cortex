use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let icon_path = manifest_dir.join("../desktop/build/icon.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());
    // Icon + VERSIONINFO stamped with the win release version — the packaged
    // exe reports the same version the installer and latest.yml carry. The
    // resource script and the version resolution are shared with
    // runtime/build.rs via pe-version-info (KOS-306); embedding is
    // target-gated inside the helper, so cross-compiles from non-Windows
    // hosts still embed.
    pe_version_info::embed(&pe_version_info::ExeInfo {
        description: "Mundus Manager",
        exe_name: "Mundus Manager.exe",
        version: pe_version_info::product_version(&manifest_dir),
        icon: Some(icon_path),
        // gpui-pre's `windows-manifest` feature already embeds the
        // application manifest (asInvoker, Win10, PerMonitorV2, SegmentHeap,
        // Common-Controls v6) — embedding a second RT_MANIFEST here would be
        // a duplicate resource.
        manifest: false,
    });
}
