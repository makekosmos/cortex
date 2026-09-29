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
            return Err(NativeAppError::Invalid("package id"));
        }
        if semver::Version::parse(&spec.version).is_err() {
            return Err(NativeAppError::Invalid("version"));
        }
        if !valid_record_executable(&spec.executable) {
            return Err(NativeAppError::Invalid("executable"));
        }
        // Refuse to update an app that is running — never delete files of a
        // live exe; the caller surfaces "close the app first".
        if let Some(current) = self.current(&spec.id) {
            if current.version != spec.version {
                let exe = self.record_executable_path(&current);
                if exe.is_file() && exe_in_use(&exe) {
                    return Err(NativeAppError::Running);
                }
            }
        }

        // Same-volume temp file contract: the caller downloads into the app
        // dir; verify archive size + sha256 against the signed catalog before
        // touching the zip.
        let metadata = fs::metadata(archive)?;
        if metadata.len() != spec.size {
            return Err(NativeAppError::SizeMismatch);
        }
        if metadata.len() > MAX_ARCHIVE {
            return Err(NativeAppError::Archive("archive too large"));
        }
        if !eq_hash(&hex_hash(&fs::read(archive)?), &spec.sha256) {
            return Err(NativeAppError::HashMismatch);
        }

        let app_dir = self.app_dir(&spec.id);
        fs::create_dir_all(&app_dir)?;
        let counter = STAGING_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let staging = app_dir.join(format!(".staging-{}-{counter}", std::process::id()));
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
        // Antivirus/indexers can transiently hold handles under a fresh dir;
        // mirror the package-store retry loop before giving up.
        let mut renamed = false;
        for attempt in 0..24 {
            match fs::rename(&staging, &target) {
                Ok(()) => {
                    renamed = true;
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied && attempt < 23 => {
                    std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
                }
                Err(error) => {
                    let _ = fs::remove_dir_all(&staging);
                    return Err(error.into());
                }
            }
        }
        if !renamed {
            let _ = fs::remove_dir_all(&staging);
            return Err(NativeAppError::Archive("staging rename timed out"));
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
            catalog_sequence: spec.catalog_sequence,
        };
        // Atomic pointer flip: install.json is the single source of truth.
        write_owner_only_json(&self.state_path(&spec.id), &record)
            .map_err(|error| NativeAppError::State(error.to_string()))?;
        // Remove superseded versions + stale staging dirs — only after the
        // pointer moved, so a failed cleanup never strands the install.
        self.cleanup_stale(&spec.id, &spec.version);
        Ok(record)
    }

    fn cleanup_stale(&self, id: &str, keep_version: &str) {
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
            if path.is_dir()
                && (name.starts_with(".staging-")
                    || name.starts_with(".download-")
                    || semver::Version::parse(name).is_ok())
            {
                let _ = fs::remove_dir_all(&path);
            }
        }
    }

    /// Extract `archive` into `staging` under the package-store safety rules:
    /// entry-count and expanded-size limits, enclosed paths only, no
    /// traversal, no duplicates, no symlinks or reparse points, no reserved
    /// device names. The catalog-declared executable must exist as a file.
    fn extract_verified(
        &self,
        archive: &Path,
        staging: &Path,
        spec: &NativeInstallSpec,
    ) -> Result<()> {
        let file = retry_io(|| fs::File::open(archive))?;
        let mut zip = ZipArchive::new(file)?;
        if zip.len() > MAX_ENTRIES {
            return Err(NativeAppError::Archive("too many entries"));
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut total: u64 = 0;
        let mut executable_found = false;
        for index in 0..zip.len() {
            let mut file = zip.by_index(index)?;
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
            let mut out = fs::File::create(&target)?;
            io::copy(&mut file, &mut out)?;
            if normalized == spec.executable {
                executable_found = true;
            }
        }
        if !executable_found {
            return Err(NativeAppError::Archive("missing entry executable"));
        }
        Ok(())
    }
}
