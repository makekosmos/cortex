//! One-way migration of pre-rename dev data dirs into the shared assets root.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::{
    default_shared_assets_base, default_shared_assets_root, models_dir, same_path_or_text,
    shared_assets_root, tools_dir, MODELS_DIR, TOOLS_DIR,
};

pub(super) fn merge_legacy_dir_into_shared(legacy: &Path, shared: &Path) -> io::Result<bool> {
    if !legacy.is_dir() || same_path_or_text(legacy, shared) {
        return Ok(false);
    }
    if !shared.exists() {
        if let Some(parent) = shared.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(legacy, shared)?;
        return Ok(true);
    }

    let mut changed = false;
    fs::create_dir_all(shared)?;
    for entry in fs::read_dir(legacy)? {
        let entry = entry?;
        let source = entry.path();
        let destination = shared.join(entry.file_name());
        if destination.exists() {
            if source.is_dir() && destination.is_dir() {
                changed |= merge_legacy_dir_into_shared(&source, &destination)?;
                if fs::read_dir(&source)?.next().is_none() {
                    fs::remove_dir(&source)?;
                    changed = true;
                }
            } else if source.is_file() && destination.is_file() {
                if same_file_contents(&source, &destination)? {
                    fs::remove_file(&source)?;
                } else {
                    fs::rename(&source, unique_legacy_destination(&destination))?;
                }
                changed = true;
            }
            continue;
        }
        fs::rename(&source, &destination)?;
        changed = true;
    }
    if fs::read_dir(legacy)?.next().is_none() {
        fs::remove_dir(legacy)?;
        changed = true;
    }
    Ok(changed)
}

fn same_file_contents(left: &Path, right: &Path) -> io::Result<bool> {
    if left.metadata()?.len() != right.metadata()?.len() {
        return Ok(false);
    }

    let mut left = fs::File::open(left)?;
    let mut right = fs::File::open(right)?;
    let mut left_buf = [0; 64 * 1024];
    let mut right_buf = [0; 64 * 1024];
    loop {
        let left_read = left.read(&mut left_buf)?;
        let right_read = right.read(&mut right_buf)?;
        if left_read != right_read || left_buf[..left_read] != right_buf[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn unique_legacy_destination(destination: &Path) -> PathBuf {
    let mut i = 1;
    loop {
        let candidate = destination.with_extension(format!(
            "{}legacy-{i}",
            destination
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!("{ext}."))
                .unwrap_or_default()
        ));
        if !candidate.exists() {
            return candidate;
        }
        i += 1;
    }
}

pub fn migrate_legacy_assets(data_dir: &Path) -> io::Result<bool> {
    let shared_root = shared_assets_root(data_dir);
    let mut changed = false;
    let mut roots = vec![data_dir.to_path_buf()];
    if std::env::var("MUNDUS_LOCAL_STT_DIR").is_err()
        && !cfg!(test)
        && same_path_or_text(&shared_root, &default_shared_assets_root())
    {
        if let Ok(entries) = fs::read_dir(default_shared_assets_base()) {
            for entry in entries {
                let path = entry?.path();
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                // MIGRATION(KOS-267): pre-rename dev worktrees
                let is_dev_dir = name.starts_with("Mundus-dev") || name.starts_with("Kosmos-dev");
                if path.is_dir() && is_dev_dir {
                    roots.push(path);
                }
            }
        }
    }

    for root in roots {
        if same_path_or_text(&root, &shared_root) {
            continue;
        }
        changed |= merge_legacy_dir_into_shared(&root.join(MODELS_DIR), &models_dir(data_dir))?;
        changed |= merge_legacy_dir_into_shared(&root.join(TOOLS_DIR), &tools_dir(data_dir))?;
        #[cfg(windows)]
        {
            changed |= merge_legacy_dir_into_shared(
                &root.join(super::runtime::VULKAN_TOOLS_DIR),
                &super::runtime::vulkan_tools_dir(data_dir),
            )?;
        }
    }
    Ok(changed)
}
