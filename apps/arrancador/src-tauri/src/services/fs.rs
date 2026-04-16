use std::path::Path;

pub trait FileSystem {
    fn exists(&self, path: &Path) -> bool;
}

#[derive(Clone, Copy, Default)]
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}
