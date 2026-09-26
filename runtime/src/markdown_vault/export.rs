//! `exportVault` port from `markdown-file-operations.ts`: validate the whole
//! plan first (dupes, caps, path safety), then write text entries and copy
//! resolved local sources through the grant root handle.

use crate::handle_relative_fs::{self, RootHandle};
use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

use super::{MARKDOWN_FILE_MAX_BYTES, MARKDOWN_IMAGE_MAX_BYTES};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultExportFile {
    Text {
        components: Vec<String>,
        content: String,
    },
    Copy {
        components: Vec<String>,
        source: PathBuf,
    },
}

impl VaultExportFile {
    fn key(&self) -> String {
        let joined = match self {
            Self::Text { components, .. } | Self::Copy { components, .. } => components.join("/"),
        };
        if cfg!(windows) {
            joined.to_lowercase()
        } else {
            joined
        }
    }
    fn components(&self) -> &[String] {
        match self {
            Self::Text { components, .. } | Self::Copy { components, .. } => components,
        }
    }
}

/// Write every planned file beneath `root`. All validation happens before the
/// first write, mirroring the TS two-pass structure — a malformed plan fails
/// without touching the vault. Returns the number of files actually written
/// (a failed asset copy is skipped, like `defaultAssetCopy` returning false).
pub fn export_files(root: &RootHandle, files: &[VaultExportFile]) -> io::Result<usize> {
    if files.len() > super::MARKDOWN_VAULT_MAX_FILES + super::MARKDOWN_VAULT_MAX_IMAGES {
        return Err(invalid("vault payload exceeds file count limit"));
    }
    let mut written_keys = HashSet::with_capacity(files.len());
    let mut text_count = 0usize;
    let mut image_count = 0usize;
    for file in files {
        if !written_keys.insert(file.key()) {
            return Err(invalid("duplicate vault path"));
        }
        match file {
            VaultExportFile::Text { content, .. } => {
                text_count += 1;
                if text_count > super::MARKDOWN_VAULT_MAX_FILES
                    || content.len() as u64 > MARKDOWN_FILE_MAX_BYTES
                {
                    return Err(invalid("vault text limit exceeded"));
                }
            }
            VaultExportFile::Copy { .. } => {
                image_count += 1;
                if image_count > super::MARKDOWN_VAULT_MAX_IMAGES {
                    return Err(invalid("vault image limit exceeded"));
                }
            }
        }
    }
    let mut exported = 0usize;
    for file in files {
        let components = file.components();
        if components.len() > 1 {
            let parents: Vec<&str> = components[..components.len() - 1]
                .iter()
                .map(String::as_str)
                .collect();
            handle_relative_fs::mkdir_relative(root, &parents)?;
        }
        let target: Vec<&str> = components.iter().map(String::as_str).collect();
        match file {
            VaultExportFile::Text { content, .. } => {
                handle_relative_fs::write_relative(
                    root,
                    &target,
                    content.as_bytes(),
                    MARKDOWN_FILE_MAX_BYTES as usize,
                )?;
                exported += 1;
            }
            VaultExportFile::Copy { source, .. } => {
                if copy_source(root, &target, source)? {
                    exported += 1;
                }
            }
        }
    }
    Ok(exported)
}

/// `defaultAssetCopy` — read a resolved local file (bounded) and write it
/// under the vault root. Failures are non-fatal: the entry is skipped.
fn copy_source(root: &RootHandle, target: &[&str], source: &std::path::Path) -> io::Result<bool> {
    let attempt = || -> io::Result<bool> {
        let meta = std::fs::metadata(source)?;
        if !meta.is_file() || meta.len() > MARKDOWN_IMAGE_MAX_BYTES {
            return Ok(false);
        }
        let bytes = std::fs::read(source)?;
        handle_relative_fs::write_relative(
            root,
            target,
            &bytes,
            MARKDOWN_IMAGE_MAX_BYTES as usize,
        )?;
        Ok(true)
    };
    Ok(attempt().unwrap_or(false))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_string())
}
