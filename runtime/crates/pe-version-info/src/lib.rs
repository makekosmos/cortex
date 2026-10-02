//! Shared build-time helper that embeds an icon, a VERSIONINFO resource and
//! the default application manifest into every Windows exe we ship (KOS-306).
//!
//! Defender's ML weighs the absence of version metadata heavily; a bare exe
//! with empty CompanyName/FileDescription looks like a dropper. Both
//! `runtime/build.rs` (mundus-engine) and `manager-gpui/build.rs` call
//! [`embed`] so the resource script is written once, here.

use std::env;
use std::fs;
use std::path::PathBuf;

/// OS-facing company/brand strings — mirrors `runtime/src/brand.rs` and
/// `desktop/scripts/brand.mjs`.
pub const COMPANY_NAME: &str = "Kazui";
pub const LEGAL_COPYRIGHT: &str = "Copyright (C) Kazui";

/// Everything VERSIONINFO needs to describe one shipped executable.
pub struct ExeInfo<'a> {
    /// `FileDescription` / `ProductName` — e.g. "Mundus Engine".
    pub display_name: &'a str,
    /// `InternalName` / `OriginalFilename` — the packaged exe file name.
    pub exe_name: &'a str,
    /// `FileVersion` / `ProductVersion`, `X.Y.Z` → `X.Y.Z.0` in the
    /// fixed-field parts.
    pub version: [u32; 3],
    /// Optional `.ico` embedded as the exe's icon.
    pub icon: Option<PathBuf>,
}

/// Embed the resources. No-op on non-Windows targets — the decision is keyed
/// on `CARGO_CFG_TARGET_OS` (the target), not `cfg!`, so cross-compiles to
/// Windows (cargo-xwin) still embed.
// Build-time helper: a failed write/compile must abort the cargo build —
// panicking here is the correct failure mechanism, not an error return the
// caller's build.rs could silently swallow.
#[allow(clippy::panic, clippy::unwrap_used)]
pub fn embed(info: &ExeInfo) {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set in build scripts"));
    let rc_path = out_dir.join(format!("{}.rc", info.exe_name));
    fs::write(&rc_path, resource_script(info)).unwrap_or_else(|e| panic!("write {rc_path:?}: {e}"));
    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_required()
        .unwrap_or_else(|e| panic!("embed resources for {}: {e}", info.exe_name));
}

fn resource_script(info: &ExeInfo) -> String {
    let [major, minor, patch] = info.version;
    let version = format!("{major}.{minor}.{patch}");
    let icon_line = info.icon.as_ref().map_or(String::new(), |icon| {
        format!(
            "1 ICON \"{}\"",
            icon.display().to_string().replace('\\', "\\\\")
        )
    });
    format!(
        r#"{icon_line}
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "{COMPANY_NAME}"
            VALUE "FileDescription", "{display_name}"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "{exe_name}"
            VALUE "LegalCopyright", "{LEGAL_COPYRIGHT}"
            VALUE "OriginalFilename", "{exe_name}"
            VALUE "ProductName", "{display_name}"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
        display_name = info.display_name,
        exe_name = info.exe_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_script_carries_every_required_versioninfo_key() {
        let rc = resource_script(&ExeInfo {
            display_name: "Mundus Engine",
            exe_name: "mundus-engine.exe",
            version: [1, 2, 3],
            icon: Some(PathBuf::from(r"C:\icons\app.ico")),
        });
        for key in [
            "CompanyName",
            "FileDescription",
            "FileVersion",
            "InternalName",
            "LegalCopyright",
            "OriginalFilename",
            "ProductName",
            "ProductVersion",
        ] {
            assert!(rc.contains(key), "missing {key}");
        }
        assert!(rc.contains("FILEVERSION 1,2,3,0"));
        assert!(rc.contains("PRODUCTVERSION 1,2,3,0"));
        assert!(rc.contains("\"FileVersion\", \"1.2.3\""));
        assert!(rc.contains(r#"1 ICON "C:\\icons\\app.ico""#));
    }
}
