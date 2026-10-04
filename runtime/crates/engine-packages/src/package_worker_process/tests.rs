#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::package_manifest::PackageKind;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::mem::size_of;
    use zip::{write::FileOptions, ZipWriter};

    include!("tests_validation.rs");
    include!("tests_windows.rs");
    #[cfg(target_os = "macos")]
    include!("tests_macos.rs");
}
