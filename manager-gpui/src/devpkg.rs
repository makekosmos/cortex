//! Development-package path resolution — parity with Electron's
//! dev-archive.ts/dev-packages.ts. Given a `.kspkg` archive or a package
//! source directory, derive `{id, version, archive_path}` for
//! `packages.install_development`. The Engine re-validates the manifest.
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};

const EOCD_SIGNATURE: u32 = 0x06054b50;
const CENTRAL_SIGNATURE: u32 = 0x02014b50;
const LOCAL_SIGNATURE: u32 = 0x04034b50;
const MAX_ARCHIVE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_ENTRY_SIZE: u64 = 1024 * 1024;
const MAX_ENTRIES: u16 = 4096;

pub struct DevTarget {
    pub id: String,
    pub version: String,
    pub archive_path: PathBuf,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-_".contains(&b))
        && id.as_bytes()[0].is_ascii_alphanumeric()
}

/// Resolve a user-supplied path (archive file or package source dir).
pub fn resolve(path: &Path) -> Result<DevTarget, String> {
    if path.extension().is_some_and(|e| e == "kspkg") {
        let (id, version) = archive_manifest(path)?;
        return Ok(DevTarget {
            id,
            version,
            archive_path: path.to_path_buf(),
        });
    }
    let manifest_path = ["package.manifest.json", "manifest.json"]
        .iter()
        .map(|name| path.join(name))
        .find(|p| p.is_file())
        .ok_or("В каталоге нет package.manifest.json")?;
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(&manifest_path).map_err(|e| format!("Манифест не читается: {e}"))?,
    )
    .map_err(|_| "Манифест не является JSON".to_string())?;
    let id = manifest["id"].as_str().unwrap_or_default().to_string();
    let version = manifest["version"].as_str().unwrap_or_default().to_string();
    if !valid_id(&id) || version.is_empty() || version.len() > 64 {
        return Err("Манифест не содержит корректные id/version".into());
    }
    let release = path.join("release");
    let suffix = format!("-{version}.kspkg");
    let archive = std::fs::read_dir(&release)
        .map_err(|_| "В каталоге нет release/ с собранным .kspkg".to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.is_file()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(&suffix))
        })
        .ok_or("В release/ нет архива этой версии")?;
    Ok(DevTarget {
        id,
        version,
        archive_path: archive,
    })
}

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// `manifest.json` entry inside a `.kspkg` (plain zip). Mirrors
/// readPackageArchiveManifest: EOCD → central directory → local header.
fn archive_manifest(file: &Path) -> Result<(String, String), String> {
    let bytes = std::fs::read(file).map_err(|e| format!("Архив не читается: {e}"))?;
    if bytes.len() as u64 > MAX_ARCHIVE_SIZE || bytes.len() < 22 {
        return Err("Некорректный .kspkg".into());
    }
    let tail = bytes.len() - 22;
    let floor = bytes.len().saturating_sub(65557);
    let eocd = (floor..=tail)
        .rev()
        .find(|&i| u32le(&bytes, i) == EOCD_SIGNATURE)
        .ok_or("Не найден каталог zip")?;
    let entries = u16le(&bytes, eocd + 10);
    if entries > MAX_ENTRIES {
        return Err("Слишком много записей в архиве".into());
    }
    let mut offset = u32le(&bytes, eocd + 16) as usize;
    for _ in 0..entries {
        if offset + 46 > bytes.len() || u32le(&bytes, offset) != CENTRAL_SIGNATURE {
            return Err("Повреждённый каталог zip".into());
        }
        let method = u16le(&bytes, offset + 10);
        let compressed = u32le(&bytes, offset + 20) as usize;
        let uncompressed = u32le(&bytes, offset + 24) as usize;
        let name_len = u16le(&bytes, offset + 28) as usize;
        let extra_len = u16le(&bytes, offset + 30) as usize;
        let comment_len = u16le(&bytes, offset + 32) as usize;
        let local = u32le(&bytes, offset + 42) as usize;
        let name = String::from_utf8_lossy(&bytes[offset + 46..offset + 46 + name_len]);
        if name == "manifest.json" {
            if uncompressed as u64 > MAX_ENTRY_SIZE || local + 30 > bytes.len() {
                return Err("Манифест в архиве повреждён".into());
            }
            if u32le(&bytes, local) != LOCAL_SIGNATURE {
                return Err("Повреждённый локальный заголовок zip".into());
            }
            let start = local
                + 30
                + u16le(&bytes, local + 26) as usize
                + u16le(&bytes, local + 28) as usize;
            let raw: Vec<u8> = match method {
                0 => bytes[start..start + compressed].to_vec(),
                8 => {
                    let mut out = Vec::with_capacity(uncompressed);
                    flate2::read::DeflateDecoder::new(&bytes[start..start + compressed])
                        .take(MAX_ENTRY_SIZE + 1)
                        .read_to_end(&mut out)
                        .map_err(|_| "Манифест в архиве не распаковывается".to_string())?;
                    out
                }
                _ => return Err("Неподдерживаемое сжатие манифеста".into()),
            };
            let manifest: Value = serde_json::from_slice(&raw)
                .map_err(|_| "Манифест не является JSON".to_string())?;
            let id = manifest["id"].as_str().unwrap_or_default().to_string();
            let version = manifest["version"].as_str().unwrap_or_default().to_string();
            if !valid_id(&id) || version.is_empty() || version.len() > 64 {
                return Err("Манифест не содержит корректные id/version".into());
            }
            return Ok((id, version));
        }
        offset += 46 + name_len + extra_len + comment_len;
    }
    Err("В архиве нет manifest.json".into())
}
