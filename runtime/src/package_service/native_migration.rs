// MIGRATION(KOS-267): remove after 2026-11-01 — legacy package-store and
// bundled-component detection plus the native install hand-off.

impl PackageService {
    // MIGRATION(KOS-267): remove after 2026-11-01
    //
    // First-start migration for users upgrading from 0.9.x: if the package
    // store still has an installed record for a legacy bundled/kspkg app, or
    // the bundled component dir survived the installer wipe, auto-install the
    // native app from the signed catalog. Idempotent (the marker records
    // per-id outcomes), never blocks Engine startup, failures are logged and
    // retried on the next start.
    pub async fn migrate_legacy_native_apps(&self) {
        const LEGACY: &[(&str, &str)] = &[
            ("com.kosmos.agenda", "agenda"),
            ("com.kosmos.memoria", "memoria"),
            ("com.kosmos.dictation", "dictation"),
        ];
        // A first-ever start has no persisted catalog yet — pull it once so
        // the upgrade path does not wait for the next refresh cycle.
        let has_catalog = Self::lock(&self.state).catalog.is_some();
        if !has_catalog && self.refresh_catalog().await.is_err() {
            tracing::warn!(
                target: "native_apps",
                "legacy app migration: catalog refresh failed; will retry next start"
            );
        }
        let marker_path = self.root.join("native-apps-migration.json");
        let mut marker = read_owner_only_json::<serde_json::Value>(&marker_path)
            .ok()
            .filter(|value| value["schema_version"].as_u64() == Some(1))
            .unwrap_or_else(|| serde_json::json!({"schema_version": 1, "apps": {}}));
        let done = marker["apps"].clone();
        for (id, dir) in LEGACY {
            if done[id]["installed"].as_bool().unwrap_or(false) {
                continue;
            }
            if self
                .native_apps
                .as_ref()
                .and_then(|store| store.current(id))
                .is_some()
            {
                marker["apps"][id] = serde_json::json!({"installed": true, "reason": "already-installed"});
                let _ = write_owner_only_json(&marker_path, &marker);
                continue;
            }
            let had_package = self
                .store
                .list()
                .unwrap_or_default()
                .iter()
                .any(|package| &package.id == id);
            let had_component = legacy_component_dirs(dir)
                .iter()
                .any(|path| path.is_dir());
            if !had_package && !had_component {
                marker["apps"][id] = serde_json::json!({"installed": true, "reason": "not-present"});
                let _ = write_owner_only_json(&marker_path, &marker);
                continue;
            }
            match self.install_native_app(id, None).await {
                Ok(_) => {
                    // Remove the legacy package-store records now that the
                    // native install is live; worker stop is handled inside.
                    for package in self
                        .store
                        .list()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|package| &package.id == id)
                    {
                        if let Err(error) = self
                            .uninstall_with_worker_stop(&package.id, &package.version)
                            .await
                        {
                            tracing::warn!(
                                target: "native_apps",
                                %error,
                                "legacy package uninstall failed"
                            );
                        }
                    }
                    marker["apps"][id] = serde_json::json!({"installed": true});
                    let _ = write_owner_only_json(&marker_path, &marker);
                    tracing::info!(target: "native_apps", %id, "migrated legacy app to native install");
                }
                Err(error) => {
                    tracing::warn!(
                        target: "native_apps",
                        %id,
                        %error,
                        "legacy app migration failed; will retry on next start"
                    );
                }
            }
        }
    }
}

// MIGRATION(KOS-267): remove after 2026-11-01
//
// Bundled-component dirs from 0.9.x installs: `$INSTDIR\resources\components`.
/// `$INSTDIR` was `%LOCALAPPDATA%\Programs\Kosmos`; test-only env override
/// `MUNDUS_LEGACY_COMPONENTS_DIR` supplies a direct `components/` dir.
fn legacy_component_dirs(name: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = crate::brand::env("LEGACY_COMPONENTS_DIR") {
        if !dir.is_empty() {
            roots.push(PathBuf::from(dir));
        }
    }
    // Unit-test builds never probe real product dirs — a dev machine with a
    // real 0.9.x install must not flip the migration branch.
    if !cfg!(test) {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            roots.push(
                PathBuf::from(local)
                    .join("Programs")
                    .join("Kosmos")
                    .join("resources")
                    .join("components"),
            );
        }
    }
    roots.into_iter().map(|root| root.join(name)).collect()
}
