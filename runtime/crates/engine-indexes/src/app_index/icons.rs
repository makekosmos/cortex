// Icon extraction + PNG cache.
//
// Win32: ExtractIconExW (or через .lnk → target.exe) → HICON → GetIconInfo →
// GDI DIB → BGRA→RGBA swap → `image` crate PNG encode → файл в icon cache dir.
//
// UWP: Package.GetLogo() → RandomAccessStreamReference → OpenReadAsync →
// DataReader → bytes (обычно уже PNG) → файл.
//
// Storage: `<icon_cache_dir>/<sha256(exec_path)[..16]>.png`. Идемпотентно —
// если файл существует, не пересоздаём.
//
// Discovery не пишет PNG. Sources сохраняют `App.icon_source`, а `AppIndex::rescan`
// вызывает `ensure_icon` последовательным throttled loop'ом.

#[cfg(windows)]
use crate::app_index::app::IconSource;
use crate::app_index::app::{App, AppKind};
use crate::app_index::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub fn cached_icon_path(cache_dir: &Path, exec_path: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(exec_path.as_bytes());
    let hash = hex_short(&hasher.finalize());
    cache_dir.join(format!("{hash}.png"))
}

fn hex_short(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(32);
    for b in &bytes[..8] {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Гарантировать наличие иконки на диске. Возвращает путь.
/// Если файл уже существует — no-op (быстро).
/// При любой ошибке экстрактора — возвращает Err БЕЗ записи placeholder,
/// чтобы следующий rescan повторил попытку (UI рендерит placeholder в DOM).
pub fn ensure_icon(cache_dir: &Path, app: &App) -> Result<String> {
    let path = cached_icon_path(cache_dir, &app.exec_path);
    if path.exists() {
        return Ok(path.to_string_lossy().to_string());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match app.kind {
        AppKind::Win32 => {
            #[cfg(target_os = "windows")]
            {
                if let Some(IconSource::StartMenuLnk {
                    lnk_path,
                    target_path,
                }) = app.icon_source.as_ref()
                {
                    return ensure_icon_for_lnk(cache_dir, Path::new(lnk_path), target_path);
                }
                win32::extract_to_png(&app.exec_path, &path).map_err(|e| {
                    tracing::debug!(
                        target: "app_index::icons",
                        exec_path = %app.exec_path,
                        error = %e,
                        "win32 icon extraction failed"
                    );
                    crate::app_index::AppIndexError::Other(format!("icon extraction: {e}"))
                })?;
                Ok(path.to_string_lossy().to_string())
            }
            #[cfg(not(target_os = "windows"))]
            {
                Err(crate::app_index::AppIndexError::Other(
                    "Win32 icon extraction only on Windows".into(),
                ))
            }
        }
        AppKind::Uwp => {
            #[cfg(target_os = "windows")]
            {
                if let Some(IconSource::UwpPackage { package_full_name }) = app.icon_source.as_ref()
                {
                    return ensure_icon_for_uwp_package(
                        cache_dir,
                        &app.exec_path,
                        package_full_name,
                    );
                }
                Err(crate::app_index::AppIndexError::Other(
                    "UWP icon extraction missing package metadata".into(),
                ))
            }
            #[cfg(not(target_os = "windows"))]
            {
                Err(crate::app_index::AppIndexError::Other(
                    "UWP icon extraction only on Windows".into(),
                ))
            }
        }
        _ => Err(crate::app_index::AppIndexError::Other(format!(
            "icon extraction not implemented for kind: {:?}",
            app.kind
        ))),
    }
}

/// Win32 икон-extractor с приоритетом .lnk::icon_location над target.
/// Используется lazy из throttled icon queue — это спасает
/// Squirrel-installer apps (Discord, Slack, Teams), где target=Update.exe
/// без иконки, а icon_location в .lnk указывает на реальный exe.
#[cfg(target_os = "windows")]
pub fn ensure_icon_for_lnk(cache_dir: &Path, lnk_path: &Path, target_path: &str) -> Result<String> {
    let out = cached_icon_path(cache_dir, target_path);
    if out.exists() {
        return Ok(out.to_string_lossy().to_string());
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    win32::extract_to_png_with_lnk(lnk_path, target_path, &out)
        .map_err(|e| crate::app_index::AppIndexError::Other(format!("lnk icon extraction: {e}")))?;
    Ok(out.to_string_lossy().to_string())
}

/// UWP icon extraction для throttled queue.
/// Принимает уже открытый `Package` (избегает повторного PackageManager lookup).
/// Идемпотентно (skip if exists). При ошибке — placeholder, debug log.
#[cfg(target_os = "windows")]
pub fn ensure_icon_for_uwp(
    cache_dir: &Path,
    exec_path: &str,
    package: &windows::ApplicationModel::Package,
) -> Result<String> {
    let path = cached_icon_path(cache_dir, exec_path);
    if path.exists() {
        return Ok(path.to_string_lossy().to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match uwp::extract_logo_to_png(package, &path) {
        Ok(()) => {}
        Err(e) => {
            tracing::debug!(
                target: "app_index::icons",
                exec_path = %exec_path,
                error = %e,
                "uwp logo extraction failed, writing placeholder"
            );
            write_placeholder(&path)?;
        }
    }

    Ok(path.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
fn ensure_icon_for_uwp_package(
    cache_dir: &Path,
    exec_path: &str,
    package_full_name: &str,
) -> Result<String> {
    use windows::core::HSTRING;
    use windows::Management::Deployment::PackageManager;

    let package_manager = PackageManager::new()
        .map_err(|e| crate::app_index::AppIndexError::Other(format!("PackageManager::new: {e}")))?;
    let empty = HSTRING::new();
    let full_name = HSTRING::from(package_full_name);
    let package = package_manager
        .FindPackageByUserSecurityIdPackageFullName(&empty, &full_name)
        .map_err(|e| {
            crate::app_index::AppIndexError::Other(format!(
                "FindPackageByUserSecurityIdPackageFullName: {e}"
            ))
        })?;

    ensure_icon_for_uwp(cache_dir, exec_path, &package)
}

/// 1×1 прозрачный PNG (67 байт). Placeholder если extractor не сработал.
#[cfg(target_os = "windows")]
const PLACEHOLDER_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

#[cfg(target_os = "windows")]
fn write_placeholder(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, PLACEHOLDER_PNG)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Win32 extraction
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
#[path = "icons/win32.rs"]
mod win32;

#[cfg(target_os = "windows")]
#[path = "icons/uwp.rs"]
mod uwp;
