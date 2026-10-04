#[allow(clippy::panic)]
pub mod tests {
    use super::*;
    use crate::package_manifest::PermissionRequest;
    #[cfg(test)]
    use crate::{
        catalog::PackageRevocation,
        package_manifest::{TargetOs, TargetRuntime},
    };
    use sha2::{Digest, Sha256};
    use std::{fs::File, io::Write};
    #[cfg(test)]
    use tempfile::tempdir;
    use zip::write::FileOptions;

    include!("tests/fixtures.rs");
    include!("tests/catalog.rs");
    include!("tests/lifecycle.rs");
    include!("tests/disclosure.rs");
    include!("tests/launch.rs");
    include!("tests/bridge.rs");
    include!("tests/manifests.rs");

    #[cfg(all(windows, target_arch = "x86_64"))]
    include!("native_tests.rs");
    #[cfg(all(windows, target_arch = "x86_64"))]
    include!("native_icon_tests.rs");
    #[cfg(all(windows, target_arch = "x86_64"))]
    include!("native_migration_tests.rs");
}
