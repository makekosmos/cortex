//! Startup sweep of interrupted write leftovers inside the package store
//! (KOS-301). Called from `PackageStore::new` — the write-path open — so a
//! crash between temp write and rename never litters the store.
//!
//! Shapes, all produced by this module or `package_service` next to it:
//! - `.download-{id}-{version}.kspkg` — `install_from_url` payload, deleted
//!   after install on the happy path only;
//! - `.{name}.tmp.{pid}` — `write_owner_only_json` temps (state.json,
//!   bridge-config.json, …) and `<name>.json.tmp` from the catalog writer;
//! - `.{hash}.{pid}.tmp` in `blobs/` — the blob copy temp before rename;
//! - `.staging-{hash}-{n}/` — crashed extract dirs in the store root.

use std::path::Path;

use crate::data_dir::temp_sweep::{self, LEFTOVER_GRACE};

pub(crate) fn sweep_stale_leftovers(root: &Path) {
    temp_sweep::sweep(root, LEFTOVER_GRACE, |name, is_dir| {
        if is_dir {
            name.starts_with(".staging-")
        } else {
            (name.starts_with(".download-") && name.ends_with(".kspkg"))
                || (name.starts_with('.') && name.contains(".tmp."))
                || name.ends_with(".json.tmp")
        }
    });
    temp_sweep::sweep(&root.join("blobs"), LEFTOVER_GRACE, |name, is_dir| {
        !is_dir && name.starts_with('.') && name.ends_with(".tmp")
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime};

    fn aged(dir: &Path, name: &str) {
        let path = dir.join(name);
        fs::write(&path, b"x").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(2 * 60 * 60))
            .unwrap();
    }

    #[test]
    fn removes_every_writer_shape_keeps_real_files() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("blobs")).unwrap();
        aged(root.path(), ".download-pkg-1.0.0.kspkg");
        aged(root.path(), ".state.json.tmp.42");
        aged(root.path(), "catalog.json.tmp");
        aged(&root.path().join("blobs"), ".deadbeef.42.tmp");
        fs::write(root.path().join("state.json"), b"{}").unwrap();
        fs::write(root.path().join("catalog.json"), b"{}").unwrap();
        fs::write(root.path().join("blobs").join("deadbeef.kspkg"), b"x").unwrap();
        // A file named like a leftover but sitting in `unpacked/` is out of
        // scope — only root and blobs/ are ours.
        fs::create_dir(root.path().join("unpacked")).unwrap();
        aged(&root.path().join("unpacked"), ".download-x-1.0.0.kspkg");

        sweep_stale_leftovers(root.path());

        let mut remaining: Vec<String> = fs::read_dir(root.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        remaining.sort();
        assert_eq!(
            remaining,
            vec!["blobs", "catalog.json", "state.json", "unpacked"]
        );
        assert!(root
            .path()
            .join("unpacked/.download-x-1.0.0.kspkg")
            .is_file());
        assert_eq!(fs::read_dir(root.path().join("blobs")).unwrap().count(), 1);
    }

    #[test]
    fn young_leftovers_survive() {
        let root = tempfile::tempdir().unwrap();
        // Just-created — could belong to an install running right now.
        fs::write(root.path().join(".download-pkg-1.0.0.kspkg"), b"x").unwrap();
        fs::create_dir(root.path().join(".staging-live-0")).unwrap();

        sweep_stale_leftovers(root.path());

        assert!(root.path().join(".download-pkg-1.0.0.kspkg").is_file());
        assert!(root.path().join(".staging-live-0").is_dir());
    }
}
