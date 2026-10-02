// `packages.open` — the dispatch-side open for installed App packages
// (KOS-299). It reuses the launch-lease machinery of `POST /v1/apps/launch`
// through the `LaunchSurface` the Engine API server shares in: same
// definition registration, same `resolve_app` checks, same typed-grant
// lease, one mint path — two entry points.
//
// Every call is a new session: a second «Открыть» opens a second browser
// tab, and two tabs sharing one lease would revoke each other on close.
// The returned `launch_url` carries the single-use bootstrap code in its
// fragment — `broker_token` never reaches the Manager.

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

    /// Mint a fresh launch lease for an App package — every `packages.open`
    /// is a new tab, and a new tab must own a new session: two tabs sharing
    /// one lease would revoke each other on close. The reply carries only
    /// `launch_url` (with the one-time bootstrap code in the fragment — the
    /// browser never sends it to the Engine) plus the row identity; the
    /// Manager needs nothing else.
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
        let lease = surface
            .leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .create_with_typed_grant(
                crate::package_launch::AssetGrant {
                    id: package.id.clone(),
                    version: package.version.clone(),
                    hash: package.hash.clone(),
                },
                resolved.grant,
            )
            .map_err(|_| PackageOpenError::AtCapacity)?;
        let code = lease
            .bootstrap_code
            .as_deref()
            .ok_or(PackageOpenError::LaunchFailed)?;
        Ok(Value::Object(
            [
                ("id".into(), Value::String(package.id.clone())),
                ("version".into(), Value::String(package.version.clone())),
                (
                    "name".into(),
                    Value::String(package.manifest.name().to_owned()),
                ),
                (
                    "launch_url".into(),
                    Value::String(format!(
                        "http://{}:{}/v1/apps/assets/{}/{}#launch={}&code={}&pkg={}&v={}",
                        crate::package_launch::package_origin_host(&package.id),
                        surface.http_port,
                        lease.asset_token,
                        package.manifest.entrypoint(),
                        lease.launch_id,
                        code,
                        package.id,
                        package.version,
                    )),
                ),
            ]
            .into_iter()
            .collect(),
        ))
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
