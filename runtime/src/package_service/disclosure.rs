fn package_disclosure(manifest: &VersionedManifest) -> PackageDisclosure {
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
    PackageDisclosure {
        id: manifest.id().to_owned(),
        name: manifest.name().to_owned(),
        version: manifest.version().to_owned(),
        kind: manifest.kind().clone(),
        publisher: manifest.publisher().to_owned(),
        capabilities: manifest.permissions().to_vec(),
        data,
        mappings,
    }
}
