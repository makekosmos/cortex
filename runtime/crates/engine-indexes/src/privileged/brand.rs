//! Product-facing names for the privileged Engine service.
//!
//! Every product-derived string for the service is sourced from
//! `crate::brand` (the single source of product identity). Code outside this
//! module must not embed product names.

/// Windows service name. Stable across product releases — renaming this
/// orphans the registered service, so only change it together with an
/// explicit service migration.
pub const SERVICE_NAME: &str = crate::brand::SYSTEM_SERVICE_NAME;

/// Named pipe the service listens on.
pub const PIPE_NAME: &str = crate::brand::SYSTEM_SERVICE_PIPE;

/// Service display name shown in services.msc.
pub const SERVICE_DISPLAY_NAME: &str = "Mundus Privileged Service";

/// Service description shown in services.msc.
pub const SERVICE_DESCRIPTION: &str =
    "Privileged operations for the Mundus Engine: managed hosts-file blocks and fast NTFS \
     indexing. Granted once via UAC; runs only fixed operations requested over a restricted \
     local named pipe.";

/// Display brand shown to the user (service description, install dir).
pub const PRODUCT_NAME: &str = crate::brand::PRODUCT_NAME;

/// File name of the service binary copy. Deliberately different from
/// `mundus-engine.exe` so `taskkill /IM mundus-engine.exe` in the
/// installer/uninstaller never kills the running service.
pub const SERVICE_BINARY_NAME: &str = "mundus-privileged-service.exe";
