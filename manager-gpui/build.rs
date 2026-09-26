use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let versions_path = manifest_dir.join("../desktop/release-versions.json");
    let icon_path = manifest_dir.join("../desktop/build/icon.ico");
    println!("cargo:rerun-if-changed={}", versions_path.display());
    println!("cargo:rerun-if-changed={}", icon_path.display());
    windows::embed(&versions_path, &icon_path);
}

/// Windows resource script: application icon plus VERSIONINFO stamped with the
/// win release version from desktop/release-versions.json — the packaged exe
/// reports the same version the installer and latest.yml carry. Guarded on
/// CARGO_CFG_TARGET_OS (the target), not cfg! — the build script itself always
/// compiles for the host, so this also runs when cross-compiling to Windows.
mod windows {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    pub fn embed(versions_path: &Path, icon_path: &Path) {
        if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
            return;
        }
        let version = release_version(versions_path);
        let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        let rc_path = out_dir.join("kosmos-manager.rc");
        fs::write(&rc_path, resource_script(icon_path, version))
            .expect("write Kosmos Manager resource script");
        embed_resource::compile(&rc_path, embed_resource::NONE)
            .manifest_required()
            .expect("embed Kosmos Manager icon and version resources");
    }

    fn resource_script(icon: &Path, [major, minor, patch]: [u32; 3]) -> String {
        let version = format!("{major}.{minor}.{patch}");
        let icon = icon.display().to_string().replace('\\', "\\\\");
        format!(
            r#"1 ICON "{icon}"
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
            VALUE "CompanyName", "Kazui"
            VALUE "FileDescription", "Kosmos Manager"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "Kosmos Manager.exe"
            VALUE "LegalCopyright", "Copyright (C) Kazui"
            VALUE "OriginalFilename", "Kosmos Manager.exe"
            VALUE "ProductName", "Kosmos Manager"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
        )
    }

    fn release_version(path: &Path) -> [u32; 3] {
        let raw = fs::read_to_string(path).expect("read desktop/release-versions.json");
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
}
