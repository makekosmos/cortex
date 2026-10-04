use super::*;

// ponytail: serialize typed mutations globally; split by object id if contention appears.
static DATA_MUTATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(super) async fn execute_data_request(
    ark: &ArkHost,
    request: DataRequest,
) -> Result<serde_json::Value, &'static str> {
    use crate::runtime_grants::DataRequest::*;
    match request {
        ListObjects {
            type_id,
            type_version,
            fields,
            relations,
            limit,
            cursor,
            ..
        } => {
            if cursor.is_some() {
                return Err("invalid-request");
            }
            let response = ark
                .request("list_objects", serde_json::Value::Null)
                .await
                .map_err(|_| "unavailable")?;
            if !response.ok {
                return Err("unavailable");
            }
            let links = read_links(ark, &relations).await?;
            let values = response
                .data
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|value| {
                    object_identity(value)
                        .is_some_and(|(id, version)| id == type_id && version == type_version)
                })
                .take(limit as usize)
                .map(|value| project_object(value, &fields, &links))
                .collect();
            Ok(serde_json::Value::Array(values))
        }
        ReadObject {
            type_id,
            type_version,
            object_id,
            fields,
            relations,
        } => {
            let value = ark_value(ark, "get_object", serde_json::json!({"id":object_id})).await?;
            if !object_identity(&value)
                .is_some_and(|(id, version)| id == type_id && version == type_version)
            {
                return Err("not-found");
            }
            let links = read_links(ark, &relations).await?;
            Ok(project_object(value, &fields, &links))
        }
        CreateObject {
            type_id,
            type_version,
            object_id,
            fields,
            links,
        } => {
            let _mutation = DATA_MUTATION.lock().await;
            if !links.is_empty() {
                return Err("invalid-request");
            }
            if let Some(id) = object_id.as_deref() {
                let existing = ark_value(ark, "get_object", serde_json::json!({"id":id})).await?;
                if !existing.is_null() {
                    return Err("conflict");
                }
            }
            let now = chrono::Utc::now().to_rfc3339();
            let object = apply_fields(
                serde_json::json!({
                    "id": object_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    "typeId": type_id, "typeVersion": type_version, "title": "",
                    "contentJson": {}, "propsJson": {}, "createdAt": now,
                    "updatedAt": now, "deletedAt": null
                }),
                fields,
            )?;
            upsert(ark, object).await
        }
        UpdateObject {
            type_id,
            type_version,
            object_id,
            fields,
            links,
            expected_hlc,
        } => {
            let _mutation = DATA_MUTATION.lock().await;
            if !links.is_empty() || expected_hlc.is_some() {
                return Err("invalid-request");
            }
            let existing =
                ark_value(ark, "get_object", serde_json::json!({"id":object_id})).await?;
            if !object_identity(&existing)
                .is_some_and(|(id, version)| id == type_id && version == type_version)
            {
                return Err("not-found");
            }
            let mut object = apply_fields(existing, fields)?;
            object["updatedAt"] = serde_json::Value::String(chrono::Utc::now().to_rfc3339());
            upsert(ark, object).await
        }
        DeleteObject {
            type_id,
            type_version,
            object_id,
            expected_hlc,
        } => {
            let _mutation = DATA_MUTATION.lock().await;
            if expected_hlc.is_some() {
                return Err("invalid-request");
            }
            let existing =
                ark_value(ark, "get_object", serde_json::json!({"id":object_id})).await?;
            if !object_identity(&existing)
                .is_some_and(|(id, version)| id == type_id && version == type_version)
            {
                return Err("not-found");
            }
            ark_value(
                ark,
                "delete_object",
                serde_json::json!({"id":object_id,"device_id":"package-worker"}),
            )
            .await
        }
        Subscribe { .. } | Link { .. } | Batch { .. } => Err("unavailable"),
    }
}

fn project_object(
    mut value: serde_json::Value,
    fields: &[String],
    links: &[serde_json::Value],
) -> serde_json::Value {
    let allows = |field: &str| {
        fields
            .iter()
            .any(|allowed| allowed == field || allowed == "props.*" && field.starts_with("props."))
    };
    if let Some(object) = value.as_object_mut() {
        let object_id = object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let object_links = links
            .iter()
            .filter(|link| {
                link.get("sourceObjectId")
                    .and_then(serde_json::Value::as_str)
                    == object_id.as_deref()
            })
            .cloned()
            .collect();
        object.insert("links".into(), serde_json::Value::Array(object_links));
        if !allows("title") {
            object.remove("title");
        }
        if !allows("content") {
            object.remove("contentJson");
            object.remove("content_json");
        }
        for key in ["propsJson", "props_json"] {
            if let Some(props) = object
                .get_mut(key)
                .and_then(serde_json::Value::as_object_mut)
            {
                props.retain(|name, _| allows(&format!("props.{name}")));
            }
        }
    }
    value
}

async fn read_links(
    ark: &ArkHost,
    relations: &[String],
) -> Result<Vec<serde_json::Value>, &'static str> {
    if relations.is_empty() {
        return Ok(Vec::new());
    }
    let links = ark_value(ark, "list_object_links", serde_json::Value::Null).await?;
    Ok(links
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|link| {
            link.get("linkType")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|relation| relations.iter().any(|allowed| allowed == relation))
        })
        .collect())
}

async fn upsert(
    ark: &ArkHost,
    object: serde_json::Value,
) -> Result<serde_json::Value, &'static str> {
    ark_value(
        ark,
        "upsert_object",
        serde_json::json!({"object":object,"device_id":"package-worker"}),
    )
    .await
}

async fn ark_value(
    ark: &ArkHost,
    operation: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, &'static str> {
    let response = ark
        .request(operation, params)
        .await
        .map_err(|_| "unavailable")?;
    response.ok.then_some(response.data).ok_or("unavailable")
}

fn object_identity(value: &serde_json::Value) -> Option<(&str, &str)> {
    Some((
        value
            .get("typeId")
            .or_else(|| value.get("type_id"))?
            .as_str()?,
        value
            .get("typeVersion")
            .or_else(|| value.get("type_version"))?
            .as_str()?,
    ))
}

fn apply_fields(
    mut object: serde_json::Value,
    fields: Vec<crate::runtime_grants::FieldInput>,
) -> Result<serde_json::Value, &'static str> {
    for field in fields {
        let value = serde_json::to_value(field.value).map_err(|_| "invalid-request")?;
        match field.field_id.as_str() {
            "title" => object["title"] = value,
            "content" => object["contentJson"] = value,
            field_id if field_id.starts_with("props.") => {
                object["propsJson"]
                    .as_object_mut()
                    .ok_or("invalid-response")?
                    .insert(field_id[6..].to_owned(), value);
            }
            _ => return Err("invalid-request"),
        }
    }
    Ok(object)
}
