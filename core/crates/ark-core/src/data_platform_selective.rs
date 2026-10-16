#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Full,
    Metadata,
    None,
}

#[derive(Debug, Clone, Default)]
pub struct SelectiveSyncProfile {
    pub rules: HashMap<(String, String), SelectiveSyncRule>,
}

#[derive(Debug, Clone)]
pub struct SelectiveSyncRule {
    pub mode: SyncMode,
    pub filter: FilterV1,
}

impl SelectiveSyncProfile {
    pub fn rule(&self, kind: &str, id: &str) -> SyncMode {
        self.rules
            .get(&(kind.to_string(), id.to_string()))
            .map(|rule| rule.mode)
            .unwrap_or(SyncMode::None)
    }

    pub fn set_rule(&mut self, kind: &str, id: &str, mode: SyncMode) {
        self.set_rule_with_filter(kind, id, mode, FilterV1::match_all());
    }

    pub fn set_rule_with_filter(&mut self, kind: &str, id: &str, mode: SyncMode, filter: FilterV1) {
        self.rules.insert(
            (kind.to_string(), id.to_string()),
            SelectiveSyncRule { mode, filter },
        );
    }

    fn rule_for(&self, kind: &str, id: &str) -> Option<&SelectiveSyncRule> {
        self.rules.get(&(kind.to_string(), id.to_string()))
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
        let Some(rule) = entity_rule(profile, entity) else {
            continue;
        };
        if rule_matches_entity(rule, entity) {
            selected.insert((entity.entity_type.clone(), entity.id.clone()));
            modes.insert((entity.entity_type.clone(), entity.id.clone()), rule.mode);
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
                if let Some(rule) =
                    entity_rule(profile, object).filter(|rule| rule_matches_entity(rule, object))
                {
                    if selected.insert(("object".into(), target.into())) {
                        modes.insert(("object".into(), target.into()), rule.mode);
                        changed = true;
                    }
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

fn entity_rule<'a>(
    profile: &'a SelectiveSyncProfile,
    entity: &crate::types::SyncEntity,
) -> Option<&'a SelectiveSyncRule> {
    if entity.entity_type == "object_type" {
        return profile.rule_for("type", &entity.id);
    }
    if entity.entity_type == "object" {
        return profile.rule_for(
            "type",
            entity
                .data
                .get("typeId")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
    }
    if entity.entity_type.starts_with("usage_") {
        return profile.rule_for("dataset", "usage");
    }
    None
}

fn rule_matches_entity(rule: &SelectiveSyncRule, entity: &crate::types::SyncEntity) -> bool {
    rule.mode != SyncMode::None
        && (entity.entity_type == "object_type" || rule.filter.matches(&entity.data))
}

fn project_entity(entity: &crate::types::SyncEntity, mode: SyncMode) -> crate::types::SyncEntity {
    let mut projected = entity.clone();
    if entity.entity_type == "object" {
        projected.data.retain(|key, _| {
            !matches!(
                key.as_str(),
                "localState"
                    | "local_state"
                    | "secret"
                    | "secrets"
                    | "credentials"
                    | "credential"
                    | "vaultRoot"
            )
        });
        if let Some(Value::Object(props)) = projected.data.get_mut("propsJson") {
            props.retain(|key, _| {
                !matches!(
                    key.as_str(),
                    "localState"
                        | "local_state"
                        | "secret"
                        | "secrets"
                        | "credentials"
                        | "credential"
                        | "vaultRoot"
                )
            });
        }
    }
    if mode == SyncMode::Metadata && entity.entity_type == "object" {
        projected.data.retain(|key, _| {
            matches!(
                key.as_str(),
                "typeId"
                    | "typeVersion"
                    | "title"
                    | "createdAt"
                    | "updatedAt"
                    | "deletedAt"
                    | "propsJson"
            )
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
    (
        phase,
        entity.entity_type.clone(),
        entity.id.clone(),
        entity.hlc.clone(),
    )
}
