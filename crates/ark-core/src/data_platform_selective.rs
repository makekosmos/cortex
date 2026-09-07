#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Full,
    Metadata,
    None,
}

#[derive(Debug, Clone, Default)]
pub struct SelectiveSyncProfile {
    pub rules: HashMap<(String, String), SyncMode>,
}

impl SelectiveSyncProfile {
    pub fn rule(&self, kind: &str, id: &str) -> SyncMode {
        self.rules
            .get(&(kind.to_string(), id.to_string()))
            .copied()
            .unwrap_or(SyncMode::None)
    }

    pub fn set_rule(&mut self, kind: &str, id: &str, mode: SyncMode) {
        self.rules.insert((kind.to_string(), id.to_string()), mode);
    }
}

/// Apply one deterministic projection to batch, reconnect, live, and tombstone callers.
pub fn filter_entities_for_profile(
    profile: &SelectiveSyncProfile,
    entities: &[crate::types::SyncEntity],
) -> Vec<crate::types::SyncEntity> {
    let mut selected = HashSet::new();
    let mut modes = HashMap::new();
    for entity in entities {
        let mode = entity_mode(profile, entity);
        if mode != SyncMode::None {
            selected.insert((entity.entity_type.clone(), entity.id.clone()));
            modes.insert((entity.entity_type.clone(), entity.id.clone()), mode);
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for link in entities.iter().filter(|e| e.entity_type == "object_link") {
            let source = link
                .data
                .get("sourceObjectId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !selected.contains(&("object".into(), source.into())) {
                continue;
            }
            let target = link
                .data
                .get("targetObjectId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if let Some(object) = entities
                .iter()
                .find(|e| e.entity_type == "object" && e.id == target)
            {
                let mode = entity_mode(profile, object);
                if mode != SyncMode::None && selected.insert(("object".into(), target.into())) {
                    modes.insert(("object".into(), target.into()), mode);
                    changed = true;
                }
            }
        }
    }

    let mut result = entities
        .iter()
        .filter_map(|entity| {
            let key = (entity.entity_type.clone(), entity.id.clone());
            if !selected.contains(&key) || entity.entity_type == "object_link" {
                return None;
            }
            let mode = modes.get(&key).copied().unwrap_or(SyncMode::None);
            Some(project_entity(entity, mode))
        })
        .collect::<Vec<_>>();
    for link in entities.iter().filter(|e| e.entity_type == "object_link") {
        let source = link
            .data
            .get("sourceObjectId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let target = link
            .data
            .get("targetObjectId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if selected.contains(&("object".into(), source.into()))
            && selected.contains(&("object".into(), target.into()))
        {
            result.push(project_entity(link, SyncMode::Metadata));
        }
    }
    result.sort_by_key(entity_sort_key);
    result
}

fn entity_mode(profile: &SelectiveSyncProfile, entity: &crate::types::SyncEntity) -> SyncMode {
    if entity.entity_type == "object_type" {
        return match profile.rule("type", &entity.id) {
            SyncMode::None => SyncMode::None,
            _ => SyncMode::Full,
        };
    }
    if entity.entity_type == "object" {
        return profile.rule(
            "type",
            entity
                .data
                .get("typeId")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
    }
    if entity.entity_type.starts_with("usage_") {
        return profile.rule("dataset", "usage");
    }
    SyncMode::None
}

fn project_entity(entity: &crate::types::SyncEntity, mode: SyncMode) -> crate::types::SyncEntity {
    let mut projected = entity.clone();
    if entity.entity_type == "object" {
        projected.data.retain(|key, _| {
            !matches!(
                key.as_str(),
                "localState" | "local_state" | "secret" | "secrets" | "credentials" | "credential" | "vaultRoot"
            )
        });
        if let Some(Value::Object(props)) = projected.data.get_mut("propsJson") {
            props.retain(|key, _| {
                !matches!(
                    key.as_str(),
                    "localState" | "local_state" | "secret" | "secrets" | "credentials" | "credential" | "vaultRoot"
                )
            });
        }
    }
    if mode == SyncMode::Metadata && entity.entity_type == "object" {
        projected.data.retain(|key, _| {
            matches!(key.as_str(), "typeId" | "typeVersion" | "title" | "createdAt" | "updatedAt" | "deletedAt" | "propsJson")
        });
    }
    projected
}

fn entity_sort_key(entity: &crate::types::SyncEntity) -> (u8, String, String, String) {
    let phase = match entity.entity_type.as_str() {
        "object_type" => 0,
        "object" => 1,
        "object_link" => 2,
        _ if entity.entity_type.starts_with("usage_") => 3,
        _ => 1,
    };
    (phase, entity.entity_type.clone(), entity.id.clone(), entity.hlc.clone())
}
