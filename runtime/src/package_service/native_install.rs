// Native app install/update/uninstall — download + verified extraction
// through the signed catalog.

impl PackageService {
    /// Catalog-facing install/update: downloads the archive to a temp file
    /// inside the app dir (same volume as the install target), verifies it
    /// against the signed catalog, then flips the pointer.
    pub async fn install_native_app(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<NativeAppSummary, PackageError> {
        let apps_root = self.native_store()?.root().to_path_buf();
        let (entry, native) = {
            let mut state = Self::lock(&self.state);
            Self::native_entry(&mut state, id, version)?
        };
        if host_native_target() != Some(native.target.as_str()) {
            return Err(PackageError::Invalid);
        }
        // Same-version reinstall is a no-op — the pointer already agrees.
        if self
            .native_store()?
            .current(entry.manifest.id())
            .is_some_and(|record| record.version == *entry.manifest.version())
        {
            let state = Self::lock(&self.state);
            let record = self.native_store()?.current(entry.manifest.id());
            return Ok(self.native_summary(&state, record.as_ref(), entry.manifest.id()));
        }

        let app_dir = apps_root.join(entry.manifest.id());
        retry_io(|| fs::create_dir_all(&app_dir)).map_err(|_| PackageError::Persistence)?;
        let download = app_dir.join(format!(
            ".download-{}-{}",
            std::process::id(),
            entry.manifest.version()
        ));

        let parsed = reqwest::Url::parse(&entry.archive_url).map_err(|_| PackageError::Invalid)?;
        let fetched: Vec<u8> = if parsed.scheme() == "file" && cfg!(debug_assertions) {
            // file:// archive URLs exist only in debug/test catalogs.
            let path = parsed
                .to_file_path()
                .map_err(|_| PackageError::Invalid)?;
            fs::read(path).map_err(|_| PackageError::Invalid)?
        } else {
            if parsed.scheme() != "https"
                || parsed.username() != ""
                || parsed.password().is_some()
                || parsed.fragment().is_some()
            {
                return Err(PackageError::Invalid);
            }
            let response = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|_| PackageError::Invalid)?
                .get(parsed)
                .send()
                .await
                .map_err(|_| PackageError::Invalid)?;
            if !response.status().is_success() {
                return Err(PackageError::Invalid);
            }
            if response
                .content_length()
                .is_some_and(|size| size != entry.size)
            {
                return Err(PackageError::Invalid);
            }
            let bytes = response.bytes().await.map_err(|_| PackageError::Invalid)?;
            if bytes.len() as u64 != entry.size {
                return Err(PackageError::Invalid);
            }
            bytes.to_vec()
        };
        fs::write(&download, &fetched).map_err(|_| PackageError::Persistence)?;
        let result =
            self.install_native_from_path(entry.manifest.id(), entry.manifest.version(), &download);
        let _ = fs::remove_file(&download);
        result
    }

    /// Shared core for `install_native_app` and fixture tests: the archive is
    /// already a local path; the signed catalog entry still gates id, version,
    /// sha256, size and the zip layout.
    pub fn install_native_from_path(
        &self,
        id: &str,
        version: &str,
        archive: &Path,
    ) -> Result<NativeAppSummary, PackageError> {
        let store = self.native_store()?;
        let (entry, native) = {
            let mut state = Self::lock(&self.state);
            Self::native_entry(&mut state, id, Some(version))?
        };
        if host_native_target() != Some(native.target.as_str()) {
            return Err(PackageError::Invalid);
        }
        let sequence = Self::lock(&self.state)
            .catalog
            .as_ref()
            .map(|catalog| catalog.document.sequence)
            .unwrap_or(0);
        let spec = NativeInstallSpec {
            id: id.to_owned(),
            version: version.to_owned(),
            executable: native.executable.clone(),
            sha256: entry.sha256.clone(),
            size: entry.size,
            repository: native.repository.clone(),
            release_tag: native.release_tag.clone(),
            catalog_sequence: sequence,
        };
        let record = store
            .install_archive(&spec, archive)
            .map_err(native_store_error)?;
        let state = Self::lock(&self.state);
        Ok(self.native_summary(&state, Some(&record), id))
    }

    /// Uninstall removes the whole `<id>` dir — record and every version —
    /// refusing while the app is running. User data is never touched.
    pub fn uninstall_native_app(&self, id: &str) -> Result<(), PackageError> {
        self.native_store()?.uninstall(id).map_err(native_store_error)
    }

    /// Absolute executable path for the installed app (env override first —
    /// development only), or `Invalid` when not installed.
    pub fn native_app_executable(&self, id: &str) -> Result<PathBuf, PackageError> {
        if let Some(env_var) = crate::native_apps::executable_env_override(id) {
            if let Some(path) = crate::brand::env_os(env_var).map(PathBuf::from) {
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
        self.native_store()?
            .executable_path(id)
            .ok_or(PackageError::Invalid)
    }

    /// Launch the installed app. Refuses when the record is revoked by the
    /// signed revocation feed.
    pub fn open_native_app(&self, id: &str) -> Result<(), PackageError> {
        let record = self.native_store()?.current(id).ok_or(PackageError::Invalid)?;
        {
            let state = Self::lock(&self.state);
            if state.trust.as_ref().is_some_and(|trust| {
                trust.is_package_revoked(&record.id, &record.version, &record.sha256)
            }) {
                return Err(PackageError::Invalid);
            }
        }
        let executable = self.native_app_executable(id)?;
        #[cfg(windows)]
        let mut command = {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            let mut command = std::process::Command::new(&executable);
            command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
            command
        };
        #[cfg(not(windows))]
        let mut command = std::process::Command::new(&executable);
        command.spawn().map_err(|_| PackageError::Invalid)?;
        Ok(())
    }
}
