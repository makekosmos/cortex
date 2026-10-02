//! Shared build-time helper that embeds an icon, a VERSIONINFO resource and
//! (optionally) an application manifest into every Windows exe we ship
//! (KOS-306).
//!
//! Defender's ML weighs the absence of version metadata heavily; a bare exe
//! with empty CompanyName/FileDescription looks like a dropper. Both
//! `runtime/build.rs` (mundus-engine) and `manager-gpui/build.rs` call
//! [`embed`] so the resource script is written once, here.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Brand strings shared by every shipped exe — the contract test
/// `desktop/scripts/release-build-contract.test.mjs` compares these with
/// `desktop/scripts/brand.mjs`, `runtime/src/brand.rs` and
/// `desktop/build/installer.nsi` and fails on drift.
pub const PRODUCT_NAME: &str = "Mundus";
pub const COMPANY_NAME: &str = "Kazui";
pub const LEGAL_COPYRIGHT: &str = "Copyright (C) Kazui";

/// Application manifest (RT_MANIFEST id 1): `asInvoker` + `uiAccess=false`
/// and the Windows 10/11 supportedOS GUID (10 and 11 share it). No
/// Common-Controls v6 dependency and no DPI/heap settings — the Engine is a
/// background binary that creates no windows, and any setting beyond
/// identity/elevation/OS-version would change runtime behaviour.
const APPLICATION_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
    </application>
  </compatibility>
</assembly>
"#;

/// Everything the resources need to describe one shipped executable.
pub struct ExeInfo<'a> {
    /// `FileDescription` — e.g. "Mundus Engine". `ProductName` is always the
    /// shared [`PRODUCT_NAME`], so every exe reports the same product as
    /// the installer.
    pub description: &'a str,
    /// `InternalName` / `OriginalFilename` — the packaged exe file name.
    pub exe_name: &'a str,
    /// `FileVersion` / `ProductVersion` — always
    /// [`product_version()`], resolved in both callers the same way.
    pub version: [u32; 3],
    /// Optional `.ico` embedded as the exe's icon.
    pub icon: Option<PathBuf>,
    /// Embed [`APPLICATION_MANIFEST`]. `false` when a dependency already
    /// ships one — `Mundus Manager.exe` gets its manifest (asInvoker +
    /// Win10 + PerMonitorV2 + SegmentHeap + Common-Controls v6) from
    /// `gpui-pre`'s `windows-manifest` feature; embedding a second
    /// RT_MANIFEST would be a duplicate resource.
    pub manifest: bool,
}

/// The version stamped into every shipped exe — one function, one rule for
/// both callers. `MUNDUS_PRODUCT_VERSION` is set by build-backend.mjs /
/// build-package-components.mjs for packaged builds; a plain `cargo build`
/// falls back to `desktop/release-versions.json` `win`, which is exactly
/// what those scripts read — never a silent `CARGO_PKG_VERSION` drift.
/// `package_dir` is the *calling* crate's `CARGO_MANIFEST_DIR` (`runtime/` or
/// `manager-gpui/`), so both resolve the same `../desktop/` path.
// Build-time helper: a bad/missing version must abort the build — panic is
// the mechanism, an Err would be silently ignored by the caller's build.rs.
#[allow(clippy::panic, clippy::unwrap_used)]
pub fn product_version(package_dir: &Path) -> [u32; 3] {
    println!("cargo:rerun-if-env-changed=MUNDUS_PRODUCT_VERSION");
    let versions_path = package_dir.join("../desktop/release-versions.json");
    println!("cargo:rerun-if-changed={}", versions_path.display());

    match env::var("MUNDUS_PRODUCT_VERSION") {
        Ok(raw) => parse_version(&raw),
        // Same source the build scripts read — no second mechanism.
        Err(_) => parse_version(&win_version(&versions_path)),
    }
}

#[allow(clippy::panic, clippy::unwrap_used)] // see product_version
fn win_version(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path:?}: {e}"))
        .split("\"win\"")
        .nth(1)
        .and_then(|rest| rest.split('"').nth(1))
        .expect("release-versions.json must contain a win version")
        .to_owned()
}

#[allow(clippy::panic)] // see product_version
fn parse_version(raw: &str) -> [u32; 3] {
    let parts: Vec<u32> = raw
        .split('.')
        .map(|part| {
            part.parse()
                .unwrap_or_else(|_| panic!("product version must be X.Y.Z, got {raw:?}"))
        })
        .collect();
    <[u32; 3]>::try_from(parts.as_slice())
        .unwrap_or_else(|_| panic!("product version must be X.Y.Z, got {raw:?}"))
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
    fs::write(&rc_path, resource_script(info, &out_dir))
        .unwrap_or_else(|e| panic!("write {rc_path:?}: {e}"));
    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_required()
        .unwrap_or_else(|e| panic!("embed resources for {}: {e}", info.exe_name));
}

#[allow(clippy::panic, clippy::unwrap_used)] // see embed — build-time failure aborts the build
fn resource_script(info: &ExeInfo, out_dir: &Path) -> String {
    let [major, minor, patch] = info.version;
    let version = format!("{major}.{minor}.{patch}");
    let icon_line = info.icon.as_ref().map_or(String::new(), |icon| {
        format!(
            "1 ICON \"{}\"\n",
            icon.display().to_string().replace('\\', "\\\\")
        )
    });
    let manifest_line = if info.manifest {
        let manifest_path = out_dir.join(format!("{}.manifest", info.exe_name));
        fs::write(&manifest_path, APPLICATION_MANIFEST)
            .unwrap_or_else(|e| panic!("write {manifest_path:?}: {e}"));
        format!(
            "1 24 \"{}\"\n",
            manifest_path.display().to_string().replace('\\', "\\\\")
        )
    } else {
        String::new()
    };
    format!(
        r#"{icon_line}{manifest_line}1 VERSIONINFO
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
            VALUE "FileDescription", "{description}"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "{exe_name}"
            VALUE "LegalCopyright", "{LEGAL_COPYRIGHT}"
            VALUE "OriginalFilename", "{exe_name}"
            VALUE "ProductName", "{PRODUCT_NAME}"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
        description = info.description,
        exe_name = info.exe_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_script_carries_every_required_versioninfo_key() {
        let out = tempfile_dir();
        let rc = resource_script(
            &ExeInfo {
                description: "Mundus Engine",
                exe_name: "mundus-engine.exe",
                version: [1, 2, 3],
                icon: Some(PathBuf::from(r"C:\icons\app.ico")),
                manifest: true,
            },
            &out,
        );
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
        assert!(rc.contains(r#"1 ICON "C:\\icons\\app.ico""#));
        assert!(rc.contains(r#"VALUE "ProductName", "Mundus""#));
        // RT_MANIFEST (resource type 24) references the manifest file.
        assert!(rc.contains("1 24"));
        let manifest = fs::read_to_string(out.join("mundus-engine.exe.manifest")).unwrap();
        assert!(manifest.contains(r#"level="asInvoker""#));
        assert!(manifest.contains("8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a"));
    }

    #[test]
    fn manifest_is_optional_for_exes_whose_dependencies_ship_one() {
        let out = tempfile_dir();
        let rc = resource_script(
            &ExeInfo {
                description: "Mundus Manager",
                exe_name: "Mundus Manager.exe",
                version: [1, 2, 3],
                icon: None,
                manifest: false,
            },
            &out,
        );
        assert!(!rc.contains("1 24"));
    }

    #[test]
    fn version_parses_strictly() {
        assert_eq!(parse_version("1.2.3"), [1, 2, 3]);
        let bad = std::panic::catch_unwind(|| parse_version("1.2"));
        assert!(bad.is_err());
    }

    #[test]
    fn win_version_reads_the_release_file() {
        let dir = tempfile_dir();
        let file = dir.join("release-versions.json");
        fs::write(&file, r#"{"win":"0.9.8","mac":"0.9.9"}"#).unwrap();
        assert_eq!(win_version(&file), "0.9.8");
    }

    fn tempfile_dir() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = env::temp_dir().join(format!(
            "pe-version-info-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
