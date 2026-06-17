// UWP / Microsoft Store apps source — enum через PackageManager + AppListEntry.
//
// Каждый Package может содержать 0..N AppListEntry (launchable surfaces).
// Framework / resource / staged-not-installed пакеты — отфильтрованы.
//
// См. spec: `.agent/tasks/2026-05-22-app-launcher/spec.md`.

#![cfg(target_os = "windows")]

use crate::app_index::app::{App, AppKind, IconSource};
use crate::app_index::{AppIndexError, AppSource, Result};
use windows::core::HSTRING;
use windows::ApplicationModel::Package;
use windows::Management::Deployment::{PackageManager, PackageTypes};

/// Префиксы DisplayName / PackageFamilyName, которые не стоит показывать в лаунчере
/// (системные / runtime пакеты, юзер их не запускает напрямую).
const NOISE_PREFIXES: &[&str] = &[
    "Microsoft.NET.",
    "Microsoft.VCLibs.",
    "Microsoft.UI.Xaml.",
    "Windows.Immersive",
    "Microsoft.Services.Store",
    "Microsoft.WindowsAppRuntime",
    "Microsoft.DesktopAppInstaller",
];

/// Win32 epoch (1601-01-01) → Unix epoch (1970-01-01) — разница в 100ns тиках.
const FILETIME_UNIX_EPOCH_DIFF: i64 = 116_444_736_000_000_000;

pub struct UwpSource;

impl AppSource for UwpSource {
    fn name(&self) -> &'static str {
        "uwp"
    }

    fn discover(&self) -> Result<Vec<App>> {
        let pm = PackageManager::new().map_err(|e| {
            AppIndexError::Discover("uwp".into(), format!("PackageManager::new: {e}"))
        })?;

        // Empty HSTRING = current user. Main = только user-launchable, без Framework/Resource/Bundle.
        let empty = HSTRING::new();
        let packages = pm
            .FindPackagesByUserSecurityIdWithPackageTypes(&empty, PackageTypes::Main)
            .map_err(|e| {
                AppIndexError::Discover(
                    "uwp".into(),
                    format!("FindPackagesByUserSecurityIdWithPackageTypes: {e}"),
                )
            })?;

        let mut out: Vec<App> = Vec::new();

        for pkg in packages {
            match collect_entries_from_package(&pkg, &mut out) {
                Ok(()) => {}
                Err(e) => {
                    tracing::debug!(
                        target: "app_index::uwp",
                        error = %e,
                        "skipping package due to error"
                    );
                }
            }
        }

        Ok(out)
    }
}

/// Извлечь все валидные AppListEntry из одного Package, заполнить `out`.
/// Любая ошибка короткозамыкает обработку этого пакета (но не всего скана).
fn collect_entries_from_package(pkg: &Package, out: &mut Vec<App>) -> Result<()> {
    // Filter: framework / resource / unavailable.
    if pkg.IsFramework().unwrap_or(false) {
        return Ok(());
    }
    if pkg.IsResourcePackage().unwrap_or(false) {
        return Ok(());
    }
    // Status() возвращает PackageStatus; если получить не можем — скипаем.
    // У PackageStatus есть метод VerifyIsOK() → Result, который ошибочен если NotAvailable/Tampered/etc.
    match pkg.Status() {
        Ok(status) => {
            if status.VerifyIsOK().is_err() {
                return Ok(());
            }
        }
        Err(_) => return Ok(()),
    }

    let installed_unix = pkg
        .InstalledDate()
        .ok()
        .map(|dt| filetime_to_unix(dt.UniversalTime))
        .unwrap_or(0);

    // Mtime fallback — поможет invalidation если InstalledDate отсутствует.
    let mtime = if installed_unix > 0 {
        installed_unix
    } else {
        0
    };
    let package_full_name = pkg
        .Id()
        .ok()
        .and_then(|id| id.FullName().ok())
        .map(|h| h.to_string())
        .unwrap_or_default();

    let entries_op = pkg.GetAppListEntriesAsync().map_err(|e| {
        AppIndexError::Discover("uwp".into(), format!("GetAppListEntriesAsync: {e}"))
    })?;
    // Blocking — ок, мы в background scan thread.
    let entries = entries_op
        .get()
        .map_err(|e| AppIndexError::Discover("uwp".into(), format!("AppListEntries get: {e}")))?;

    for entry in entries {
        let display_info = match entry.DisplayInfo() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let name = match display_info.DisplayName() {
            Ok(h) => h.to_string(),
            Err(_) => continue,
        };
        if name.is_empty() {
            continue;
        }
        if is_noise(&name) {
            continue;
        }

        let aumid = match entry.AppUserModelId() {
            Ok(h) => h.to_string(),
            Err(_) => continue,
        };
        if aumid.is_empty() {
            continue;
        }

        let exec_path = format!("shell:AppsFolder\\{aumid}");

        // Сохраняем package metadata для throttled lazy extraction.
        // См. postmortems.md § 2026-06-09.
        out.push(App {
            id: aumid.clone(),
            name,
            exec_path,
            icon_path: None,
            icon_source: if package_full_name.is_empty() {
                None
            } else {
                Some(IconSource::UwpPackage {
                    package_full_name: package_full_name.clone(),
                })
            },
            kind: AppKind::Uwp,
            source: "uwp".into(),
            mtime,
        });
    }

    Ok(())
}

/// True если имя — известный системный/runtime пакет, который не стоит индексировать.
fn is_noise(name: &str) -> bool {
    NOISE_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// DateTime.UniversalTime (FILETIME, 100ns ticks since 1601-01-01 UTC) → unix seconds.
/// Отрицательные / pre-1970 — возвращаем 0 (sentinel).
fn filetime_to_unix(universal_time: i64) -> i64 {
    let diff = universal_time.saturating_sub(FILETIME_UNIX_EPOCH_DIFF);
    if diff <= 0 {
        0
    } else {
        diff / 10_000_000
    }
}

pub fn launch_uwp(exec_path: &str) -> Result<()> {
    // exec_path содержит уже `shell:AppsFolder\<AUMID>`.
    use std::process::Command;
    Command::new("cmd")
        .args(["/c", "start", "", exec_path])
        .spawn()
        .map_err(|e| crate::app_index::AppIndexError::Launch(format!("{e}")))?;
    Ok(())
}
