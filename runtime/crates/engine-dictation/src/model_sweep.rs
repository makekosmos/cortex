//! Startup sweep of interrupted local-model download leftovers (KOS-301).
//!
//! `local_models` downloads into `models/dictation/` (model files) and
//! `tools/dictation/` (whisper.cpp runtimes — the temp archive sits next to
//! the target dir, i.e. in the parent of `tools_dir`) via temp names —
//! `.<stem>.<archive>.download`, `*.{,tar.}part`, `.<stem>.quarantine.zip`,
//! `*.extracting/` — and removes them on the happy path only, so a crash
//! mid-download littered the dirs forever. Called from `DictationHost::new`:
//! at that point no download can be in flight in this process, and
//! [`LEFTOVER_GRACE`] covers a suspended writer.

use std::path::{Path, PathBuf};

use super::local_models::{models_dir, tools_dir};
use crate::data_dir::temp_sweep::{self, LEFTOVER_GRACE};

fn is_download_leftover(name: &str, is_dir: bool) -> bool {
    if is_dir {
        name.ends_with(".extracting")
    } else {
        name.ends_with(".download") || name.ends_with(".part") || name.ends_with(".quarantine.zip")
    }
}

pub(crate) fn sweep_stale_downloads(data_dir: &Path) {
    let dirs: Vec<PathBuf> = [models_dir(data_dir)]
        .into_iter()
        // Runtime archives are written next to the tool dir they extract
        // into (`tools/dictation/whisper.cpp` → temp in `tools/dictation/`).
        .chain(tools_dir(data_dir).parent().map(Path::to_path_buf))
        .collect();
    for dir in dirs {
        temp_sweep::sweep(&dir, LEFTOVER_GRACE, is_download_leftover);
    }
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
    fn removes_crash_leftovers_keeps_models_and_live_downloads() {
        let data = tempfile::tempdir().unwrap();
        let models = models_dir(data.path());
        fs::create_dir_all(&models).unwrap();
        aged(&models, ".whisper-base.whisper-base.bin.download");
        aged(&models, "whisper-small.bin.part");
        aged(&models, "whisper-large.tar.part");
        aged(&models, ".whisper-tiny.quarantine.zip");
        fs::write(models.join("whisper-base.bin"), b"model").unwrap();
        fs::write(models.join("whisper-live.bin.part"), b"in-flight").unwrap();

        sweep_stale_downloads(data.path());

        let mut remaining: Vec<String> = fs::read_dir(&models)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        remaining.sort();
        assert_eq!(remaining, vec!["whisper-base.bin", "whisper-live.bin.part"]);
    }
}
