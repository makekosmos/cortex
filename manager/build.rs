//! Icon, VERSIONINFO and the application manifest for the packaged exe.
//!
//! A windowed exe with no version metadata reads as a dropper to Defender, and
//! `gpui-fast` no longer ships an RT_MANIFEST of its own — it is built with
//! `windows-manifest-provided-by-host`, so the host has to provide one. The
//! `Gui` manifest is what keeps the loader from binding comctl32 v5.82: gpui's
//! backend imports `TaskDialogIndirect`, which only v6 exports, and without it
//! the process dies before `main`.

use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("a manifest dir"));
    let icon_path = manifest_dir.join("../desktop/build/icon.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());
    // Same helper as the Engine and the Manager this replaces: the release
    // version and the brand strings live in one place, and
    // `release-build-contract.test.mjs` keeps them in step with the installer.
    pe_version_info::embed(&pe_version_info::ExeInfo {
        description: "Mundus Manager",
        exe_name: "Mundus Manager.exe",
        version: pe_version_info::product_version(&manifest_dir),
        icon: Some(icon_path),
        manifest: pe_version_info::Manifest::Gui,
    });
}
