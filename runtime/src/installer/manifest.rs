//! `engine-manifest.json` validation and file hashing for `install` —
//! in-process port of `Assert-Manifest`/`Get-EngineSha256` from the removed
//! install-engine.ps1 (KOS-306). The manifest is written by
//! `desktop/scripts/engine-distribution.mjs` next to the staged payload.

use std::io::Read;
use std::path::Path;

use semver::Version;
use sha2::Digest;

// MIGRATION(KOS-267): 'kosmos-engine' manifests exist in installed Engine
// roots written by 0.9.x installers; accept both until cleanup.
const PRODUCTS: [&str; 2] = ["mundus-engine", "kosmos-engine"];

#[derive(Debug)]
pub struct ManifestFile {
    pub name: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug)]
pub struct Manifest {
    pub version: Version,
    pub files: Vec<ManifestFile>,
}

/// Strict `X.Y.Z` — the same shape the installer's pointer writes; prerelease
/// and build suffixes never reach an install root.
pub(crate) fn strict_version(name: &str) -> Option<Version> {
    let version = Version::parse(name).ok()?;
    (version.pre.is_empty() && version.build.is_empty()).then_some(version)
}

fn valid_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn valid_sha256(hex: &str) -> bool {
    hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn load(path: &Path) -> Result<Manifest, String> {
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| format!("read {path:?}: {e}"))?)
            .map_err(|e| format!("invalid engine manifest: {e}"))?;
    let err = "invalid engine manifest";
    if raw.get("schema_version").and_then(|v| v.as_u64()) != Some(1)
        || !PRODUCTS.contains(&raw.get("product").and_then(|v| v.as_str()).unwrap_or(""))
    {
        return Err(err.into());
    }
    let version = raw
        .get("version")
        .and_then(|v| v.as_str())
        .and_then(strict_version)
        .ok_or(err)?;
    let files = raw.get("files").and_then(|v| v.as_array()).ok_or(err)?;
    if files.is_empty() {
        return Err("engine manifest files are required".into());
    }
    let mut parsed = Vec::with_capacity(files.len());
    for file in files {
        let name = file.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let sha256 = file.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
        // as_u64 rejects floats like the PowerShell int/long check did.
        let size = file.get("size").and_then(|v| v.as_u64());
        if !valid_file_name(name) || !valid_sha256(sha256) || size.is_none() {
            return Err("invalid engine manifest file".into());
        }
        parsed.push(ManifestFile {
            name: name.to_owned(),
            sha256: sha256.to_ascii_lowercase(),
            size: size.unwrap_or_default(),
        });
    }
    Ok(Manifest {
        version,
        files: parsed,
    })
}

pub fn sha256_hex(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("open {path:?}: {e}"))?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("read {path:?}: {e}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Size+sha256 equality — the "file untampered" check for both the staged
/// payload and the installed copy.
pub fn verify_file(path: &Path, file: &ManifestFile) -> Result<(), String> {
    let meta =
        std::fs::metadata(path).map_err(|e| format!("engine file missing: {} ({e})", file.name))?;
    if meta.len() != file.size || sha256_hex(path)? != file.sha256 {
        return Err(format!("engine file mismatch: {}", file.name));
    }
    Ok(())
}

/// All manifest files verified under `root` — the `Test-InstalledEngine`
/// content check.
pub fn verify_installed(version_root: &Path, manifest: &Manifest) -> bool {
    manifest
        .files
        .iter()
        .all(|file| verify_file(&version_root.join(&file.name), file).is_ok())
}

/// Same name=sha256 set on both sides — the `Test-SameEngineBuild` port.
/// A same-version rebuild differs here and must be replaced.
pub fn same_build(version_root: &Path, expected: &Manifest) -> bool {
    let Ok(installed) = load(&version_root.join("engine-manifest.json")) else {
        return false;
    };
    let key = |files: &[ManifestFile]| {
        let mut entries: Vec<String> = files
            .iter()
            .map(|f| format!("{}={}", f.name, f.sha256))
            .collect();
        entries.sort();
        entries.join(";")
    };
    key(&installed.files) == key(&expected.files)
}

/// Read and validate the installed `engine-manifest.json` in a version dir.
/// Returns None on any malformed shape — a corrupt manifest is a corrupt
/// install, not a parse error to surface separately.
pub fn load_installed(version_root: &Path) -> Option<Manifest> {
    load(&version_root.join("engine-manifest.json")).ok()
}
