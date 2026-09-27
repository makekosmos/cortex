//! Markdown vault primitives ported from `shared/electron/markdown-vault.ts`
//! and `markdown-file-operations.ts` (KOS-155). All filesystem access goes
//! through `handle_relative_fs` root handles — callers never receive absolute
//! paths, and traversal/symlink rejection is inherited from that layer.

use std::io;

mod export;
mod image_dims;
mod local_image;
mod paths;
mod scan;

#[cfg(test)]
mod tests;

pub use export::{export_files, VaultExportFile};
pub use local_image::{
    local_image_mime_type, local_image_url, parse_local_image_request_url, LOCAL_IMAGE_PROTOCOL,
};
pub use paths::{parse_relative_path, resolve_vault_source_path, safe_markdown_default_name};
pub use scan::scan_root;

pub const MARKDOWN_FILE_MAX_BYTES: u64 = 5 * 1024 * 1024;
pub const MARKDOWN_IMAGE_MAX_BYTES: u64 = 10 * 1024 * 1024;
pub const MARKDOWN_VAULT_MAX_FILES: usize = 5_000;
pub const MARKDOWN_VAULT_MAX_IMAGES: usize = 5_000;
/// Walk/scan ceilings for a single vault root.
const VAULT_WALK_DEPTH: usize = 64;
const VAULT_WALK_ENTRIES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VaultTextFile {
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VaultImageFile {
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub name: String,
    /// `kosmos-local-image://file/…` URL — the Engine-servable reference apps
    /// store on imported image objects; it round-trips back through
    /// `resolve_vault_source_path` on export. Never a raw absolute path.
    #[serde(rename = "fileUrl")]
    pub file_url: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Validated relative components — used to read bytes later via
    /// `vault.read` without round-tripping the display path.
    #[serde(skip)]
    pub components: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct VaultScan {
    pub files: Vec<VaultTextFile>,
    pub images: Vec<VaultImageFile>,
}

pub fn vault_error(error: io::Error) -> String {
    match error.kind() {
        io::ErrorKind::NotFound => "not-found".to_string(),
        io::ErrorKind::PermissionDenied => "forbidden".to_string(),
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => "invalid-request".to_string(),
        _ => "unavailable".to_string(),
    }
}
