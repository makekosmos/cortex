// Native app install/update/uninstall — download from the app's own GitHub
// Releases, verify the archive against its `SHA256SUMS.txt` line, then the
// atomic extract/pointer-flip path in `native_apps`.

use crate::native_apps::releases;
use crate::native_apps::NativeInstallSpec;
use crate::package_store::eq_hash;

impl PackageService {
    /// Resolve the release for `id` (`version` pins a tag; `None` asks for
    /// the latest), stream the asset into the app dir, hash-check it against
    /// the sums line, and flip the install pointer.
    pub async fn install_native_app(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<NativeAppSummary, PackageError> {
        let probe = ReleaseProbe::new().map_err(|_| PackageError::Invalid)?;
        self.install_native_app_with(&probe, id, version).await
    }

    pub(crate) async fn install_native_app_with(
        &self,
        probe: &ReleaseProbe,
        id: &str,
        version: Option<&str>,
    ) -> Result<NativeAppSummary, PackageError> {
        let desc = crate::native_apps::app_descriptor(id).ok_or(PackageError::Invalid)?;
        let target = crate::native_apps::host_app_target().ok_or(PackageError::Invalid)?;
        let apps_root = self.native_store()?.root().to_path_buf();
        let info = match version {
            Some(version) => {
                if semver::Version::parse(version).is_err() {
                    return Err(PackageError::Invalid);
                }
                releases::fetch_tagged(probe, desc, &desc.release_tag(version), target).await
            }
            None => match releases::fetch_latest(probe, desc, target, None).await {
                Ok(releases::ReleaseCheck::Fresh { info, etag }) => {
                    Self::lock(&self.release_cache).insert(
                        desc.id.to_owned(),
                        CachedRelease {
                            info: info.clone(),
                            etag,
                            checked_at: std::time::Instant::now(),
                        },
                    );
                    Ok(info)
                }
                Ok(releases::ReleaseCheck::NotModified) => self
                    .cached_release(desc.id)
                    .ok_or(releases::ReleaseError::Unavailable),
                Err(error) => Err(error),
            },
        }
        .map_err(|_| PackageError::Invalid)?;

        // Same-version reinstall is a no-op — the pointer already agrees.
        if self
            .native_store()?
            .current(desc.id)
            .is_some_and(|record| record.version == info.version)
        {
            let record = self.native_store()?.current(desc.id);
            return Ok(Self::native_summary(
                desc,
                record.as_ref(),
                Some(&info),
                false,
                true,
            ));
        }

        let app_dir = apps_root.join(desc.id);
        retry_io(|| fs::create_dir_all(&app_dir)).map_err(|_| PackageError::Persistence)?;
        let download_path = app_dir.join(format!(
            ".download-{}-{}",
            std::process::id(),
            info.tag
        ));
        let url = releases::asset_url(&probe.base, desc.repository, &info.tag, &info.asset);
        let downloaded = releases::download(probe, &url, &download_path).await;
        let result = match downloaded {
            Ok((sha256, size)) if eq_hash(&sha256, &info.sha256) => {
                let spec = NativeInstallSpec {
                    id: desc.id.to_owned(),
                    version: info.version.clone(),
                    executable: desc.executable(target),
                    sha256: info.sha256.clone(),
                    size,
                    repository: desc.repository.to_owned(),
                    release_tag: info.tag.clone(),
                };
                self.install_native_verified(desc, &spec, &download_path)
            }
            // Hash mismatch or fetch failure — drop the partial download.
            _ => Err(PackageError::Invalid),
        };
        let _ = fs::remove_file(&download_path);
        result
    }

    /// Shared core once the archive is a local file: the spec carries the
    /// sums-verified sha256/size and the descriptor-owned executable.
    fn install_native_verified(
        &self,
        desc: &'static crate::native_apps::NativeAppDescriptor,
        spec: &NativeInstallSpec,
        archive: &Path,
    ) -> Result<NativeAppSummary, PackageError> {
        let record = self
            .native_store()?
            .install_archive(spec, archive)
            .map_err(native_store_error)?;
        Ok(Self::native_summary(
            desc,
            Some(&record),
            self.cached_release(desc.id).as_ref(),
            false,
            true,
        ))
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

    /// Launch the installed app.
    pub fn open_native_app(&self, id: &str) -> Result<(), PackageError> {
        if crate::native_apps::app_descriptor(id).is_none() {
            return Err(PackageError::Invalid);
        }
        self.native_store()?
            .current(id)
            .ok_or(PackageError::Invalid)?;
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
