fn package_disclosure(
    manifest: &VersionedManifest,
    registry: &RegistrySnapshot,
) -> PackageDisclosure {
    let (data, mappings) = match manifest {
        VersionedManifest::V2(manifest) => (
            manifest
                .data
                .access
                .iter()
                .map(|rule| DisclosureDataRule {
                    type_id: rule.type_id.clone(),
                    versions: rule.versions.clone(),
                    actions: rule.actions.clone(),
                    fields_read: rule.fields.read.clone(),
                    fields_write: rule.fields.write.clone(),
                    relations_read: rule
                        .relations
                        .as_ref()
                        .map_or_else(Vec::new, |relations| relations.read.clone()),
                    relations_write: rule
                        .relations
                        .as_ref()
                        .map_or_else(Vec::new, |relations| relations.write.clone()),
                })
                .collect(),
            manifest
                .data
                .mappings
                .iter()
                .map(|mapping| DisclosureMapping {
                    type_id: mapping.type_id.clone(),
                    direction: mapping.direction.clone(),
                    fidelity: mapping.fidelity.clone(),
                })
                .collect(),
        ),
        VersionedManifest::V1(_) => (Vec::new(), Vec::new()),
    };
    let volume_scopes = match manifest {
        VersionedManifest::V2(manifest) => manifest_digest(manifest)
            .ok()
            .and_then(|digest| compile_manifest_v2(manifest, registry, &digest).ok())
            .map(|grant| {
                let mut scopes = BTreeMap::<(String, String), BTreeSet<String>>::new();
                for rule in grant
                    .rules
                    .iter()
                    .filter(|rule| rule.actions.contains("read"))
                {
                    for registered in registry.types.iter().filter(|registered| {
                        registered.type_id == rule.type_id
                            && rule.versions.iter().any(|requirement| {
                                Version::parse(&registered.version)
                                    .ok()
                                    .zip(VersionReq::parse(requirement).ok())
                                    .is_some_and(|(version, requirement)| {
                                        requirement.matches(&version)
                                    })
                            })
                    }) {
                        scopes
                            .entry((registered.type_id.clone(), registered.version.clone()))
                            .or_default()
                            .extend(rule.fields_read.iter().cloned());
                    }
                }
                scopes
                    .into_iter()
                    .map(|((type_id, type_version), fields)| DisclosureVolumeScope {
                        type_id,
                        type_version,
                        fields: fields.into_iter().collect(),
                    })
                    .collect()
            }),
        VersionedManifest::V1(_) => Some(Vec::new()),
    };
    PackageDisclosure {
        id: manifest.id().to_owned(),
        name: manifest.name().to_owned(),
        version: manifest.version().to_owned(),
        kind: manifest.kind().clone(),
        publisher: manifest.publisher().to_owned(),
        capabilities: manifest.permissions().to_vec(),
        data,
        mappings,
        volume_scopes,
    }
}
