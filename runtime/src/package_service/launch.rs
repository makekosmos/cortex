// `packages.open` — the dispatch-side open for installed App packages
// (KOS-299). It reuses the launch-lease machinery of `POST /v1/apps/launch`
// through the `LaunchSurface` the Engine API server shares in: same
// definition registration, same `resolve_app` checks, same typed-grant
// lease, same payload — one launch path, two entry points.
//
// "Already running" is approximated by the lease itself: the Engine owns no
// window state, so a live lease is the only running-session evidence it can
// honestly report. A repeat open returns the existing (TTL-extended) lease
// with `already_running: true` instead of minting a duplicate — the host
// reopens the same session rather than stacking independent leases.

/// What `packages.open` reports. `code()` is the stable wire code appended
/// after `packages.open:` — the Manager maps it to a Russian UI message.
#[derive(Debug)]
pub(crate) enum PackageOpenError {
    /// Malformed id/version input — same contract `resolve_app` enforces.
    InvalidRequest,
    /// No installed App package carries this id (a revoked record counts as
    /// not installed — a revoked app can never launch).
    NotInstalled,
    /// The package is installed but disabled — the row's toggle is off.
    Disabled,
    /// The Engine runs without its HTTP launch surface, so no lease can be
    /// served (ws-only harnesses; never in a normal Engine).
    Unavailable,
    /// Launch-lease registry is full.
    AtCapacity,
    /// Definition registration, typed-grant compile or blob read failed.
    LaunchFailed,
}

impl PackageOpenError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid-request",
            Self::NotInstalled => "not-installed",
            Self::Disabled => "disabled",
            Self::Unavailable => "unavailable",
            Self::AtCapacity => "at-capacity",
            Self::LaunchFailed => "launch-failed",
        }
    }
}

impl PackageService {
    /// Called once by `EngineApiServer` before it serves — the registry is
    /// shared, never copied, so both entry points see the same leases.
    pub(crate) fn configure_launch_surface(&self, surface: crate::package_launch::LaunchSurface) {
        *Self::lock(&self.launch_surface) = Some(surface);
    }

    /// Mint (or reuse) the launch lease for an App package and return the
    /// same `data` payload `POST /v1/apps/launch` produces, plus
    /// `already_running` marking a reused live session.
    pub(crate) async fn open_app(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<Value, PackageOpenError> {
        // Same first step the HTTP launch route performs: package-defined
        // types must be registered before the compiled grant goes out.
        let dispatcher = Self::lock(&self.package_definition_dispatcher)
            .as_ref()
            .and_then(std::sync::Weak::upgrade);
        if let Some(dispatcher) = dispatcher {
            self.register_package_definitions(&dispatcher)
                .await
                .map_err(|_| PackageOpenError::LaunchFailed)?;
        }
        let resolved = self.resolve_app_for_open(id, version)?;
        let package = resolved.package;
        let surface = Self::lock(&self.launch_surface)
            .clone()
            .ok_or(PackageOpenError::Unavailable)?;
        let mut leases = surface
            .leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ttl = leases.ttl;
        let (lease, already_running) =
            match leases.reopen(&package.id, &package.version) {
                Some(lease) => (lease, true),
                None => (
                    leases
                        .create_with_typed_grant(
                            crate::package_launch::AssetGrant {
                                id: package.id.clone(),
                                version: package.version.clone(),
                                hash: package.hash.clone(),
                            },
                            resolved.grant,
                        )
                        .map_err(|_| PackageOpenError::AtCapacity)?,
                    false,
                ),
            };
        let mut payload =
            crate::package_launch::launch_payload(surface.http_port, &lease, &package, ttl);
        payload["data"]["already_running"] = Value::Bool(already_running);
        Ok(payload["data"].take())
    }

    /// `resolve_app` with the typed outcomes `packages.open` reports — the
    /// HTTP route keeps its coarse 404 through `resolve_app` below.
    fn resolve_app_for_open(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<AppLaunch, PackageOpenError> {
        if id.is_empty() || id.len() > 64 || id.chars().any(char::is_control) {
            return Err(PackageOpenError::InvalidRequest);
        }
        let candidates = self
            .store
            .list()
            .map_err(|_| PackageOpenError::LaunchFailed)?
            .into_iter()
            .filter(|package| {
                package.id == id
                    && matches!(package.manifest.kind(), PackageKind::App)
                    && version.is_none_or(|wanted| package.version == wanted)
            })
            .collect::<Vec<_>>();
        // A revoked record is launch-dead: report it as not installed rather
        // than disabled — the row's toggle cannot revive it.
        let mut enabled = candidates
            .into_iter()
            .filter(|package| !package.revoked)
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return Err(PackageOpenError::NotInstalled);
        }
        enabled.retain(|package| package.enabled);
        if enabled.is_empty() {
            return Err(PackageOpenError::Disabled);
        }
        enabled.sort_by(|left, right| {
            Version::parse(&right.version)
                .unwrap_or_else(|_| Version::new(0, 0, 0))
                .cmp(&Version::parse(&left.version).unwrap_or_else(|_| Version::new(0, 0, 0)))
        });
        let package = enabled.remove(0);
        let grant = self
            .ensure_typed_grant(&package)
            .map_err(|_| PackageOpenError::LaunchFailed)?;
        // Verify the immutable blob and entrypoint before minting a token —
        // the same pre-mint check `resolve_app` performs.
        self.store
            .read_blob_entry(&package, package.manifest.entrypoint())
            .map_err(|_| PackageOpenError::LaunchFailed)?;
        Ok(AppLaunch { package, grant })
    }
}
