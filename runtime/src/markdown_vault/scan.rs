//! `scanMarkdownVault` port — recursive vault listing over a grant root.
//! Reads markdown bodies inline (bounded); images get metadata + dimension
//! probes, bytes are fetched later via `vault.read`.

use crate::handle_relative_fs::{self, RootHandle};
use std::io;

use super::{
    image_dims::image_dimensions, local_image::local_image_mime_type, VaultImageFile, VaultScan,
    VaultTextFile, MARKDOWN_FILE_MAX_BYTES, MARKDOWN_IMAGE_MAX_BYTES, MARKDOWN_VAULT_MAX_FILES,
    MARKDOWN_VAULT_MAX_IMAGES, VAULT_WALK_DEPTH, VAULT_WALK_ENTRIES,
};

fn is_ignored_vault_dir(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules"
}

fn extension_of(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

/// `root_path` stays Engine-side: it only feeds `fileUrl`
/// (`kosmos-local-image://file/…`) construction and never enters a response as
/// a raw path.
pub fn scan_root(root: &RootHandle, root_path: &std::path::Path) -> io::Result<VaultScan> {
    let entries =
        handle_relative_fs::walk_entries(root, VAULT_WALK_DEPTH, VAULT_WALK_ENTRIES, &|name| {
            is_ignored_vault_dir(name)
        })?;
    let mut scan = VaultScan::default();
    for entry in entries {
        let name = entry.components.last().cloned().unwrap_or_default();
        let relative_path = entry.components.join("/");
        let ext = extension_of(&name).unwrap_or_default();
        if ext == "md" || ext == "markdown" {
            if scan.files.len() >= MARKDOWN_VAULT_MAX_FILES || entry.size > MARKDOWN_FILE_MAX_BYTES
            {
                continue;
            }
            let components: Vec<&str> = entry.components.iter().map(String::as_str).collect();
            let bytes = handle_relative_fs::read_relative(
                root,
                &components,
                MARKDOWN_FILE_MAX_BYTES as usize,
            )?;
            let content = String::from_utf8(bytes)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "file is not UTF-8"))?;
            scan.files.push(VaultTextFile {
                relative_path,
                name,
                content,
            });
            continue;
        }
        if super::local_image::LOCAL_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            if scan.images.len() >= MARKDOWN_VAULT_MAX_IMAGES
                || entry.size > MARKDOWN_IMAGE_MAX_BYTES
            {
                continue;
            }
            let components: Vec<&str> = entry.components.iter().map(String::as_str).collect();
            let bytes = handle_relative_fs::read_relative(
                root,
                &components,
                MARKDOWN_IMAGE_MAX_BYTES as usize,
            )?;
            let dimensions = image_dimensions(&bytes);
            let absolute = root_path.join(entry.components.iter().collect::<std::path::PathBuf>());
            scan.images.push(VaultImageFile {
                relative_path,
                name: name.clone(),
                file_url: super::local_image::local_image_url(&absolute.to_string_lossy()),
                mime_type: local_image_mime_type(&name).to_string(),
                size_bytes: entry.size,
                width: dimensions.map(|d| d.0),
                height: dimensions.map(|d| d.1),
                components: entry.components.clone(),
            });
        }
    }
    Ok(scan)
}
