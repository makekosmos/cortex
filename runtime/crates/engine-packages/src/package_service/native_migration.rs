// MIGRATION(KOS-267): remove after 2026-11-01 — legacy package-store and
// bundled-component detection plus the native install hand-off.

/// Per-app migration outcome, persisted in `native-apps-migration.json`.
/// `resolved` gates everything: an entry is written only once the app is
/// installed or proven absent from the upgrade — transient failures (store
/// read errors, an unreadable marker, a failed download) are never settled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationOutcome {
    resolved: bool,
    /// already-installed | installed | not-present
    reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationMarker {
    schema_version: u32,
    #[serde(default)]
    apps: HashMap<String, MigrationOutcome>,
}

/// What the installer recorded about the previous generation's bundled
/// components, written to `<Mundus local>/legacy-components.json` *before*
/// the old payload dirs are wiped — the Engine reads the marker because
/// probing `resources/components` after the wipe would answer "absent"
/// for components that were installed.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyComponentsMarker {
    schema_version: u32,
    components: HashMap<String, bool>,
}

impl PackageService {
    // MIGRATION(KOS-267): remove after 2026-11-01
    //
    // First-start migration for users upgrading from 0.9.x: when the upgrade
    // evidence (a legacy kspkg record, or the installer's
    // `legacy-components.json` marker) says a bundled app existed, install
    // the native app from its GitHub release. Idempotent — resolved ids are
    // never revisited — and undecidable evidence defers the app to the next
    // start instead of being recorded as "not-present". Runs in a background
    // task; never blocks Engine startup.
    pub async fn migrate_legacy_native_apps(&self) {
        let probe = match crate::native_apps::releases::ReleaseProbe::new() {
            Ok(probe) => probe,
            Err(error) => {
                tracing::warn!(
                    target: "native_apps",
                    %error,
                    "legacy app migration: no release probe",
                );
                return;
            }
        };
        self.migrate_legacy_native_apps_with(&probe).await;
    }

    pub async fn migrate_legacy_native_apps_with(
        &self,
        probe: &crate::native_apps::releases::ReleaseProbe,
    ) {
        let marker_path = self.root.join("native-apps-migration.json");
        let mut marker = read_owner_only_json::<MigrationMarker>(&marker_path)
            .ok()
            .filter(|marker| marker.schema_version == 1)
            .unwrap_or(MigrationMarker {
                schema_version: 1,
                apps: HashMap::new(),
            });
        // The installer writes `legacy-components.json` next to the Apps dir
        // — under the Mundus local dir in production, under the test root in
        // unit tests.
        let Some(apps_root) = self
            .native_apps
            .as_ref()
            .map(|store| store.root().to_path_buf())
        else {
            tracing::debug!(target: "native_apps", "legacy app migration: no apps root");
            return;
        };
        for desc in crate::native_apps::NATIVE_APPS {
            let id = desc.id;
            if marker.apps.get(id).is_some_and(|outcome| outcome.resolved) {
                self.cleanup_legacy_records(id).await;
                continue;
            }
            // A live native install settles the app regardless of evidence.
            if self.native_app_record(id).is_some() {
                marker.apps.insert(
                    id.to_owned(),
                    MigrationOutcome {
                        resolved: true,
                        reason: "already-installed".into(),
                    },
                );
                self.write_marker(&marker_path, &marker);
                if id == DICTATION_APP_ID {
                    // Already-installed still means Engine-managed running.
                    self.ensure_dictation_running();
                }
                self.cleanup_legacy_records(id).await;
                continue;
            }
            // Undecidable evidence defers the app: a store.list() failure or
            // an unreadable marker must never settle as "not-present".
            let had_package = match self.store.list() {
                Ok(packages) => packages.iter().any(|package| package.id == id),
                Err(error) => {
                    tracing::warn!(
                        target: "native_apps",
                        %id,
                        %error,
                        "legacy package list failed; deferring migration decision"
                    );
                    continue;
                }
            };
            let Some(had_component) = bundled_component_present(&apps_root, desc.legacy_component)
            else {
                tracing::warn!(
                    target: "native_apps",
                    %id,
                    "legacy-components marker unreadable; deferring migration decision"
                );
                continue;
            };
            if !had_package && !had_component {
                marker.apps.insert(
                    id.to_owned(),
                    MigrationOutcome {
                        resolved: true,
                        reason: "not-present".into(),
                    },
                );
                self.write_marker(&marker_path, &marker);
                continue;
            }
            // Same per-app claim a Store click takes — a user-triggered
            // install racing the migration wins the slot.
            let Some(job) = self.claim_native_job(id) else {
                tracing::info!(
                    target: "native_apps",
                    %id,
                    "legacy migration deferred: install already in flight"
                );
                continue;
            };
            let result = self.run_native_install_with(probe, desc, None).await;
            job.finish(&result);
            match result {
                Ok(_) => {
                    marker.apps.insert(
                        id.to_owned(),
                        MigrationOutcome {
                            resolved: true,
                            reason: "installed".into(),
                        },
                    );
                    self.write_marker(&marker_path, &marker);
                    self.cleanup_legacy_records(id).await;
                    tracing::info!(
                        target: "native_apps",
                        %id,
                        "migrated legacy app to native install",
                    );
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

    /// The app's live `install.json`, or `None` — read errors only log:
    /// a corrupt record must not make the migration claim "not present".
    fn native_app_record(&self, id: &str) -> Option<crate::native_apps::NativeAppInstall> {
        match self.native_apps.as_ref()?.current(id) {
            Ok(record) => record,
            Err(error) => {
                tracing::warn!(target: "native_apps", %id, %error, "native app record unreadable");
                None
            }
        }
    }

    /// Remove the app's legacy kspkg records once the native install is
    /// live. Called on every migration pass, so a failed uninstall is
    /// retried on the next start rather than orphaned forever.
    async fn cleanup_legacy_records(&self, id: &str) {
        let Ok(packages) = self.store.list() else {
            return;
        };
        for package in packages.iter().filter(|package| package.id == id) {
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
    }

    fn write_marker(&self, path: &Path, marker: &MigrationMarker) {
        if let Err(error) = write_owner_only_json(path, marker) {
            tracing::warn!(
                target: "native_apps",
                %error,
                "native-apps migration marker write failed; decision will repeat next start"
            );
        }
    }
}

// MIGRATION(KOS-267): remove after 2026-11-01
//
/// Whether the app's bundled component was present at upgrade time, per the
/// installer's `legacy-components.json` marker. `None` = the marker exists
/// but is unreadable — undecidable this run, defer; an absent marker (fresh
/// installs, old installers) answers `Some(false)`.
fn bundled_component_present(apps_root: &Path, component: &str) -> Option<bool> {
    // Dev/test escape hatch: `MUNDUS_LEGACY_COMPONENTS_DIR` points at a
    // `components/` dir to emulate what the installer would have recorded.
    // Release builds read only the installer marker.
    if cfg!(debug_assertions) {
        if let Some(dir) = crate::brand::env("LEGACY_COMPONENTS_DIR") {
            if !dir.is_empty() {
                return Some(PathBuf::from(dir).join(component).is_dir());
            }
        }
    }
    // The installer writes the marker next to `Apps/` — the Mundus local
    // dir is the Apps root's parent.
    let marker_path = apps_root.parent()?.join("legacy-components.json");
    match fs::read(&marker_path) {
        Ok(bytes) => match serde_json::from_slice::<LegacyComponentsMarker>(&bytes) {
            Ok(marker) if marker.schema_version == 1 => {
                Some(marker.components.get(component).copied().unwrap_or(false))
            }
            _ => None,
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
    }
}
