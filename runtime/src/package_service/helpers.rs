fn restored_package_record_matches(
    store: &PackageStore,
    previous: &InstalledPackage,
    id: &str,
    version: &str,
) -> bool {
    store
        .installed(id, version)
        .ok()
        .is_some_and(|restored| restored == *previous)
}
fn summary(
    package: InstalledPackage,
    worker: Option<&WorkerRuntime>,
    store: &PackageStore,
) -> PackageSummary {
    let health = worker.map(|runtime| runtime.supervisor.health(&package.id, &package.version));
    let icon_path = package
        .manifest
        .icon()
        .and_then(|icon| store.immutable_asset_path(&package, icon).ok())
        .map(|path| {
            let path = path.to_string_lossy();
            path.strip_prefix(r"\\?\")
                .unwrap_or(path.as_ref())
                .to_owned()
        });
    PackageSummary {
        id: package.id,
        name: package.manifest.name().to_owned(),
        version: package.version,
        hash: package.hash,
        catalog_sequence: package.catalog_sequence,
        kind: package.manifest.kind().clone(),
        enabled: package.enabled,
        revoked: package.revoked,
        worker_state: health
            .as_ref()
            .map(|health| health.state)
            .unwrap_or(WorkerState::Stopped),
        worker_restart_count: health
            .map(|health| health.restart_count)
            .unwrap_or_default(),
        publisher: package.manifest.publisher().to_owned(),
        icon_path,
        update_version: None,
    }
}

fn definition_documents_from_archive(
    archive_path: &Path,
    manifest: &ManifestV2,
) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    let mut archive =
        ZipArchive::new(fs::File::open(archive_path).map_err(|_| PackageError::Invalid)?)
            .map_err(|_| PackageError::Invalid)?;
    let paths = manifest
        .data
        .defines
        .iter()
        .flat_map(|definition| {
            std::iter::once(&definition.schema)
                .chain(definition.content_contract.iter())
                .chain(definition.relations.iter())
        })
        .collect::<Vec<_>>();
    let mut documents = BTreeMap::new();
    let mut total = 0usize;
    for path in paths {
        if documents.contains_key(path) {
            continue;
        }
        let mut file = archive.by_name(path).map_err(|_| PackageError::Invalid)?;
        if file.is_dir() || file.size() > 512 * 1024 {
            return Err(PackageError::Invalid);
        }
        total = total.saturating_add(file.size() as usize);
        if total > 8 * 1024 * 1024 {
            return Err(PackageError::Invalid);
        }
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes)
            .map_err(|_| PackageError::Invalid)?;
        documents.insert(path.clone(), bytes);
    }
    Ok(documents)
}

fn definition_documents_from_store(
    store: &PackageStore,
    package: &InstalledPackage,
) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    let VersionedManifest::V2(manifest) = &package.manifest else {
        return Ok(BTreeMap::new());
    };
    let mut documents = BTreeMap::new();
    for path in manifest.data.defines.iter().flat_map(|definition| {
        std::iter::once(&definition.schema)
            .chain(definition.content_contract.iter())
            .chain(definition.relations.iter())
    }) {
        if documents.contains_key(path) {
            continue;
        }
        let bytes = store.read_blob_entry(package, path)?;
        documents.insert(path.clone(), bytes);
    }
    Ok(documents)
}

fn canonical_registry_snapshot() -> Result<RegistrySnapshot, PackageError> {
    let registrations = ark_core::canonical_types::definitions::canonical_type_registrations()
        .map_err(|_| PackageError::Invalid)?;
    let mut types = Vec::new();
    for registration in registrations {
        let schema: Value =
            serde_json::from_str(&registration.schema_json).map_err(|_| PackageError::Invalid)?;
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .ok_or(PackageError::Invalid)?;
        let mut fields = ["title", "content"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        fields.extend(properties.keys().map(|name| format!("props.{name}")));

        let relations: Value = serde_json::from_str(&registration.relations_json)
            .map_err(|_| PackageError::Invalid)?;
        let relations = relations
            .as_array()
            .ok_or(PackageError::Invalid)?
            .iter()
            .map(|relation| {
                relation
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or(PackageError::Invalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let registered = RegisteredType {
            type_id: registration.type_id,
            version: registration.version,
            fields,
            relations,
        };
        types.push(registered.clone());
        types.extend(
            registration
                .aliases
                .into_iter()
                .map(|alias| RegisteredType {
                    type_id: alias.alias,
                    version: registered.version.clone(),
                    fields: registered.fields.clone(),
                    relations: registered.relations.clone(),
                }),
        );
    }
    for type_id in ["coding_submission_obj", "coding_profile_obj", "workout_obj"] {
        types.push(RegisteredType::new(type_id, "1.0.0", &[], &[]));
    }
    Ok(RegistrySnapshot::new(types))
}

fn manifest_digest(manifest: &ManifestV2) -> Result<String, PackageError> {
    let bytes = serde_json::to_vec(manifest).map_err(|_| PackageError::Invalid)?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn effective_grant_projections(grant: &LaunchGrant) -> Vec<EffectiveGrantProjection> {
    grant
        .rules
        .iter()
        .map(|rule| {
            let mut roles = std::collections::BTreeSet::new();
            if rule.actions.contains("read") {
                roles.insert(Role::Read);
            }
            if rule.actions.contains("create") || rule.actions.contains("update") {
                roles.insert(Role::Edit);
            }
            EffectiveGrantProjection {
                type_id: rule.type_id.clone(),
                version: rule.versions.join(","),
                roles,
                fields_read: rule.fields_read.clone(),
                fields_write: rule.fields_write.clone(),
                relations_read: rule.relations_read.clone(),
                relations_write: rule.relations_write.clone(),
            }
        })
        .collect()
}

fn read_bridge_configs(root: &Path) -> BridgeConfigState {
    let path = root.join("bridge-config.json");
    fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn validate_bridge_config(config: &BridgeConfig) -> Result<(), PackageError> {
    const MAX_ITEMS: usize = 64;
    let root = PathBuf::from(&config.vault_root);
    if config.vault_root.is_empty()
        || config.vault_root.len() > 4096
        || !root.is_absolute()
        || config.vault_root.starts_with(r"\\")
        || root.components().any(|part| {
            matches!(
                part,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || !fs::canonicalize(&root)
            .ok()
            .is_some_and(|path| path.is_dir())
        || config.selected_types.is_empty()
        || config.selected_types.len() > MAX_ITEMS
        || config.editable_fields.len() > MAX_ITEMS
        || config.readonly_fields.len() > MAX_ITEMS
        || !config
            .selected_types
            .iter()
            .all(|value| valid_bridge_name(value))
        || !config
            .editable_fields
            .iter()
            .all(|value| valid_bridge_name(value))
        || !config
            .readonly_fields
            .iter()
            .all(|value| valid_bridge_name(value))
    {
        return Err(PackageError::Invalid);
    }
    Ok(())
}

fn valid_bridge_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' || byte == b'.'
        })
}

fn latest_update_version(state: &State, package: &InstalledPackage) -> Option<String> {
    let catalog = state.catalog.as_ref()?;
    let installed = Version::parse(&package.version).ok()?;
    catalog
        .document
        .packages
        .iter()
        .filter(|entry| entry.manifest.id() == package.id)
        .filter(|entry| {
            !catalog.document.is_revoked(
                entry.manifest.id(),
                entry.manifest.version(),
                &entry.sha256,
            )
        })
        .filter_map(|entry| {
            let version = Version::parse(entry.manifest.version()).ok()?;
            (version > installed).then_some((version, entry.manifest.version().to_owned()))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, version)| version)
}
