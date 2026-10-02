#[allow(clippy::panic)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        package_manifest::{PermissionRequest, TargetOs, TargetRuntime},
        package_trust::{DetachedSignature, KeyTransitionDocument, PackageRevocation},
    };
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use std::{fs::File, io::Write};
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
