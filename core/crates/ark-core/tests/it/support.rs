//! Временный каталог для тестов, который падает громко, если не удалился.
//!
//! На Windows открытый файл нельзя удалить. `tempfile::TempDir` в `Drop`
//! проглатывает эту ошибку, и каталог с `ark.db` остаётся, а гейт потом
//! падает без имени теста. Здесь удаление проверяется явно: тест, который
//! закрыл базу не до конца, падает с путём к каталогу в сообщении.
//!
//! KOS-398: в каталог пишется `LEAK_OWNER` с именем потока/теста (nextest
//! выставляет его), чтобы гейт в логе назвал виновника даже когда к моменту
//! sweep процесс уже вышел и handle отпущен.

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
    // Падение здесь нужно: тест без удалённого каталога не должен пройти.
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
                    "temp dir {} (test {}) could not be removed, a file in it is still open: {error}",
                    path.display(),
                    owner.trim()
                );
            }
        }
    }
}
