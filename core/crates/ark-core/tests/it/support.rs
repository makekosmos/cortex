//! Временный каталог для тестов интеграционного бинаря (см. `src/test_support.rs`).
//!
//! На Windows открытый файл нельзя удалить. `tempfile::TempDir` в `Drop`
//! проглатывает эту ошибку, и каталог с `ark.db` остаётся, а гейт потом
//! падает без имени теста. Здесь удаление проверяется явно: тест, который
//! закрыл базу не до конца, падает с путём к каталогу в сообщении.

use std::path::{Path, PathBuf};

pub struct TestTempDir(Option<tempfile::TempDir>);

pub fn temp_dir() -> TestTempDir {
    TestTempDir(Some(tempfile::tempdir().expect("temp dir")))
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
        if let Err(error) = dir.close() {
            if !std::thread::panicking() {
                panic!(
                    "temp dir {} could not be removed, a file in it is still open: {error}",
                    path.display()
                );
            }
        }
    }
}
