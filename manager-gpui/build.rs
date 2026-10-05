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
        // KOS-347: the imago gpui pin no longer carries the `windows-manifest`
        // feature (agenda embeds its own), so the Manager manifest comes from
        // pe-version-info like the Engine's. Embedding a second RT_MANIFEST
        // would be a duplicate resource — do not re-enable gpui-pre's
        // `windows-manifest`. `Gui` restores gpui's Common-Controls v6
        // dependency: without it the exe fails to load on Windows
        // ("TaskDialogIndirect entry point not found").
        manifest: pe_version_info::Manifest::Gui,
    });
}
