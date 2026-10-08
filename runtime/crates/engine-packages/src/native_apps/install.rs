/// Gzip magic — a unix release asset is a `.tar.gz`; anything else falls
/// through to the zip extractor (which rejects non-zip inputs itself).
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

impl NativeAppStore {
    /// Install `archive` as `spec.version` and flip the current pointer.
    /// The previous version stays live until the new pointer is written;
    /// any earlier failure rolls back to a no-op. Reinstalling the same
    /// version replaces its tree but keeps a single coherent record.
    pub fn install_archive(
        &self,
        spec: &NativeInstallSpec,
        archive: &Path,
    ) -> Result<NativeAppInstall> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        if !valid_app_id(&spec.id) {
            return Err(NativeAppError::Invalid("app id"));
        }
        if semver::Version::parse(&spec.version).is_err() {
            return Err(NativeAppError::Invalid("version"));
        }
        if !valid_record_executable(&spec.executable) {
            return Err(NativeAppError::Invalid("executable"));
        }
        // Refuse to touch an app that is running — the old tree is deleted
        // before the rename lands, so a live exe must stop every install
        // shape (upgrade, downgrade, same-version repair). The pointer read
        // fails closed: an unreadable record is an error, not a free pass.
        if self.app_is_running(&spec.id)? {
            return Err(NativeAppError::Running);
        }

        // Same-volume temp file contract: the caller downloads into the app
        // dir; verify archive size + sha256 against the release's
        // SHA256SUMS.txt line before touching the zip.
        if fs::metadata(archive)?.len() > MAX_ARCHIVE {
            return Err(NativeAppError::Archive("archive too large"));
        }
        crate::file_hash::verify_size_and_sha256(archive, spec.size, &spec.sha256).map_err(
            |error| match error {
                crate::file_hash::VerifyError::Size { .. } => NativeAppError::SizeMismatch,
                crate::file_hash::VerifyError::Hash { .. } => NativeAppError::HashMismatch,
                crate::file_hash::VerifyError::Io(error) => NativeAppError::Io(error),
            },
        )?;

        let app_dir = self.app_dir(&spec.id);
        fs::create_dir_all(&app_dir)?;
        let staging = app_dir.join(unique_temp_name(".staging-"));
        let target = app_dir.join(&spec.version);

        let extracted = self.extract_verified(archive, &staging, spec);
        if let Err(error) = extracted {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        // Replace an existing same-version tree before the rename so Windows
        // `rename` (which refuses non-empty targets) works uniformly.
        if target.exists() && fs::remove_dir_all(&target).is_err() {
            let _ = fs::remove_dir_all(&staging);
            return Err(NativeAppError::Archive("cannot replace existing version"));
        }
        if let Err(error) = retry_io(|| fs::rename(&staging, &target)) {
            let _ = fs::remove_dir_all(&staging);
            return Err(error.into());
        }

        let record = NativeAppInstall {
            schema_version: STATE_FORMAT_VERSION,
            id: spec.id.clone(),
            version: spec.version.clone(),
            executable: spec.executable.clone(),
            sha256: spec.sha256.clone(),
            size: spec.size,
            repository: spec.repository.clone(),
            release_tag: spec.release_tag.clone(),
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        // Atomic pointer flip: install.json is the single source of truth.
        write_owner_only_json(&self.state_path(&spec.id), &record)
            .map_err(|error| NativeAppError::State(error.to_string()))?;
        // Remove superseded versions + stale temp dirs — only after the
        // pointer moved, so a failed cleanup never strands the install.
        self.cleanup_stale(&spec.id, &spec.version, &spec.executable);
        Ok(record)
    }

    /// KOS-301: `cleanup_stale` only runs after a successful install — an app
    /// that is never reinstalled kept its crashed `.download-*` files,
    /// `.staging-*`/`.tombstone-*` dirs and `write_owner_only_json` temps
    /// forever. Called from `NativeAppStore::new`: sweeps every app dir, but
    /// only temp-shaped names older than `LEFTOVER_GRACE` (a suspended
    /// download/install keeps its young temps). Superseded version dirs are
    /// *not* swept here — they need the exe-in-use check and are handled by
    /// the post-install `cleanup_stale`.
    fn sweep_stale_leftovers(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                continue;
            }
            crate::data_dir::temp_sweep::sweep(
                &path,
                crate::data_dir::temp_sweep::LEFTOVER_GRACE,
                |name, is_dir| {
                    if is_dir {
                        name.starts_with(".staging-") || name.starts_with(".tombstone-")
                    } else {
                        name.starts_with(".download-")
                            || (name.starts_with('.') && name.contains(".tmp."))
                    }
                },
            );
        }
    }

    /// Sweep everything that is not the live `<keep_version>` dir or the
    /// pointer: superseded version dirs, interrupted `.staging-*` /
    /// `.download-*` temp names (files AND dirs — a crashed download leaves
    /// a file) and `.tombstone-*` leftovers from uninstalls. A version dir
    /// whose executable is still mapped by a running process is kept —
    /// Windows denies the delete, and a half-deleted running tree is worse
    /// than a stale one.
    fn cleanup_stale(&self, id: &str, keep_version: &str, executable: &str) {
        let app_dir = self.app_dir(id);
        let Ok(entries) = fs::read_dir(&app_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if name == INSTALL_FILE || name == keep_version {
                continue;
            }
            if path.is_file() && name.starts_with(".download-") {
                let _ = fs::remove_file(&path);
                continue;
            }
            if !path.is_dir() {
                continue;
            }
            if !(name.starts_with(".staging-")
                || name.starts_with(".tombstone-")
                || semver::Version::parse(name).is_ok())
            {
                continue;
            }
            let exe = path.join(executable);
            if exe.is_file() && exe_in_use(&exe) {
                continue;
            }
            let _ = fs::remove_dir_all(&path);
        }
    }

    /// Extract `archive` into `staging` under the package-store safety rules:
    /// entry-count and expanded-size limits, enclosed paths only, no
    /// traversal, no duplicates, no symlinks or reparse points, no reserved
    /// device names. The descriptor-declared executable must exist as a file.
    /// Windows assets are zips; unix assets are gzipped tars — the format is
    /// sniffed from the magic bytes, never from the (random) download name.
    fn extract_verified(
        &self,
        archive: &Path,
        staging: &Path,
        spec: &NativeInstallSpec,
    ) -> Result<()> {
        let mut file = retry_io(|| fs::File::open(archive))?;
        let mut magic = [0u8; 2];
        let read = io::Read::read(&mut file, &mut magic)?;
        file.rewind()?;
        if read == magic.len() && magic == GZIP_MAGIC {
            return self.extract_tarball(file, staging, spec);
        }
        self.extract_zip(file, staging, spec)
    }

    fn extract_zip(&self, file: fs::File, staging: &Path, spec: &NativeInstallSpec) -> Result<()> {
        let mut zip = ZipArchive::new(file)?;
        if zip.len() > MAX_ENTRIES {
            return Err(NativeAppError::Archive("too many entries"));
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut total: u64 = 0;
        let mut executable_found = false;
        for index in 0..zip.len() {
            let file = zip.by_index(index)?;
            let Some(enclosed) = file.enclosed_name() else {
                return Err(NativeAppError::Archive("unsafe path"));
            };
            let Some(normalized) = normalize_path(&enclosed) else {
                return Err(NativeAppError::Archive("unsafe path"));
            };
            if !seen.insert(normalized.to_lowercase()) {
                return Err(NativeAppError::Archive("duplicate path"));
            }
            // zip unix-mode markers: symlink (0xAxxx) and other non-regular
            // kinds are rejected outright — same as the package store.
            if let Some(mode) = file.unix_mode() {
                let kind = mode & 0o170000;
                if kind != 0 && kind != 0o040000 && kind != 0o100000 {
                    return Err(NativeAppError::Archive("unsupported file type"));
                }
            }
            let target = staging.join(&normalized);
            if file.is_dir() {
                fs::create_dir_all(&target)?;
                continue;
            }
            if file.size() > MAX_EXPANDED {
                return Err(NativeAppError::Archive("expanded entry too large"));
            }
            total = total.saturating_add(file.size());
            if total > MAX_EXPANDED {
                return Err(NativeAppError::Archive("expanded archive too large"));
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            // Bound the write by the entry's declared size — a lying header
            // must not inflate past it (same rule as the package store).
            let declared = file.size();
            let mut out = fs::File::create(&target)?;
            let copied = io::copy(&mut file.take(declared.saturating_add(1)), &mut out)?;
            if copied != declared {
                return Err(NativeAppError::Archive("entry size mismatch"));
            }
            if normalized == spec.executable {
                executable_found = true;
            }
        }
        if !executable_found {
            return Err(NativeAppError::Archive("missing entry executable"));
        }
        Ok(())
    }

    /// The same extraction contract for gzipped tars (unix release assets):
    /// enclosed normalized paths, entry-count and expanded-size limits, no
    /// duplicates, and only regular files and directories — symlinks,
    /// hardlinks and device nodes are rejected. File modes are restored
    /// (capped at 0o777, no setuid) so the `.app` bundle's binaries keep
    /// their exec bit; the declared executable always lands runnable.
    fn extract_tarball(
        &self,
        file: fs::File,
        staging: &Path,
        spec: &NativeInstallSpec,
    ) -> Result<()> {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
        let mut seen: HashSet<String> = HashSet::new();
        let mut total: u64 = 0;
        let mut executable_found = false;
        for (index, entry) in tar.entries()?.enumerate() {
            if index >= MAX_ENTRIES {
                return Err(NativeAppError::Archive("too many entries"));
            }
            let entry = entry?;
            let header = entry.header().clone();
            let Some(normalized) = normalize_path(&entry.path()?) else {
                return Err(NativeAppError::Archive("unsafe path"));
            };
            if !seen.insert(normalized.to_lowercase()) {
                return Err(NativeAppError::Archive("duplicate path"));
            }
            let target = staging.join(&normalized);
            match header.entry_type() {
                tar::EntryType::Directory => fs::create_dir_all(&target)?,
                tar::EntryType::Regular => {
                    let declared = header.size().unwrap_or(u64::MAX);
                    if declared > MAX_EXPANDED {
                        return Err(NativeAppError::Archive("expanded entry too large"));
                    }
                    total = total.saturating_add(declared);
                    if total > MAX_EXPANDED {
                        return Err(NativeAppError::Archive("expanded archive too large"));
                    }
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut out = fs::File::create(&target)?;
                    let copied = io::copy(&mut entry.take(declared.saturating_add(1)), &mut out)?;
                    if copied != declared {
                        return Err(NativeAppError::Archive("entry size mismatch"));
                    }
                    let mode = header.mode().unwrap_or(0o644);
                    let mode = if normalized == spec.executable {
                        mode | 0o111
                    } else {
                        mode
                    };
                    apply_entry_mode(&target, mode)?;
                    if normalized == spec.executable {
                        executable_found = true;
                    }
                }
                _ => return Err(NativeAppError::Archive("unsupported file type")),
            }
        }
        if !executable_found {
            return Err(NativeAppError::Archive("missing entry executable"));
        }
        Ok(())
    }
}

/// Restore a tar file mode — permission bits only, setuid/sticky dropped.
/// No-op off unix, where modes are meaningless.
#[cfg(unix)]
fn apply_entry_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))
}

#[cfg(not(unix))]
fn apply_entry_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}
