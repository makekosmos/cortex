//! Engine install-root layout derived from an exe path.
//!
//! Moved verbatim from `engine::engine_versions::prune` (KOS-335) so
//! `privileged::firewall::validation` can resolve the calling user's
//! versioned install root without a back-edge into engine; engine
//! re-exports [`engine_root_of_exe`] at the old path.

use std::path::{Path, PathBuf};

use semver::Version;

use crate::brand;

/// Строгая `X.Y.Z` версия — тот же shape, что проверяет `installer::manifest`
/// (`semver::Version` шире: принимает prerelease/build
/// суффиксы, которых installer никогда не пишет).
fn strict_version(name: &str) -> Option<Version> {
    let version = Version::parse(name).ok()?;
    (version.pre.is_empty() && version.build.is_empty()).then_some(version)
}

/// Корень установки Engine (`<root>` в `<root>/versions/<v>/mundus-engine.exe`),
/// выведенный из пути запущенного exe. Возвращает None при любом отклонении
/// от формата — в частности, для dev-сборки из `target/` (`debug`/`release`
/// не являются semver), поэтому dev-run никогда ничего не чистит.
pub fn engine_root_of_exe(exe_path: &Path) -> Option<PathBuf> {
    let file_name = exe_path.file_name()?.to_str()?;
    if !file_name.eq_ignore_ascii_case(&format!("{}.exe", brand::ENGINE_BINARY_STEM)) {
        return None;
    }
    let version_dir = exe_path.parent()?;
    strict_version(version_dir.file_name()?.to_str()?)?;
    let versions_dir = version_dir.parent()?;
    if versions_dir.file_name()?.to_str()? != "versions" {
        return None;
    }
    Some(versions_dir.parent()?.to_path_buf())
}
