use std::io::Write;
use std::path::{Path, PathBuf};

pub struct TestTempDir(Option<tempfile::TempDir>);

pub fn temp_dir() -> TestTempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    if let Some(name) = std::thread::current().name() {
        let marker = dir.path().join("LEAK_OWNER");
        if let Ok(mut file) = std::fs::File::create(&marker) {
            let _ = writeln!(file, "{name}");
        }
    }
    TestTempDir(Some(dir))
}

impl TestTempDir {
    pub fn path(&self) -> &Path {
        self.0.as_ref().expect("temp dir is open").path()
    }
}

impl Drop for TestTempDir {
    #[allow(clippy::panic)]
    fn drop(&mut self) {
        let Some(dir) = self.0.take() else {
            return;
        };
        let path: PathBuf = dir.path().to_path_buf();
        let owner = std::fs::read_to_string(path.join("LEAK_OWNER"))
            .unwrap_or_else(|_| "(unknown test)".into());
        if let Err(error) = dir.close() {
            if !std::thread::panicking() {
                panic!(
                    "test {} left files open in {}: {error}",
                    owner.trim(),
                    path.display(),
                );
            }
        }
    }
}
