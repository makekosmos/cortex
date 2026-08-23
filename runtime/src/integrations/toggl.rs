use super::*;

pub(crate) fn toggl_time_entry_object(entry: &Value) -> Result<Value, String> {
    let external_id = entry
        .get("id")
        .and_then(Value::as_i64)
        .ok_or("Toggl time entry Р±РµР· id")?;
    let started_at = entry
        .get("start")
        .and_then(Value::as_str)
        .ok_or("Toggl time entry Р±РµР· start")?;
    let updated_at = entry
        .get("at")
        .and_then(Value::as_str)
        .unwrap_or(started_at);
    let title = entry
        .get("description")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Toggl Track");
    Ok(json!({
        "id": format!("toggl-time-entry:{external_id}"),
        "typeId": TIME_ENTRY_TYPE_ID,
        "title": title,
        "contentJson": {},
        "propsJson": {
            "startedAt": started_at,
            "endedAt": entry.get("stop").cloned().unwrap_or(Value::Null),
            "durationSeconds": entry.get("duration").cloned().unwrap_or(Value::Null),
            "billable": entry.get("billable").cloned().unwrap_or(Value::Bool(false)),
            "projectId": entry.get("project_id").or_else(|| entry.get("pid")).cloned().unwrap_or(Value::Null),
            "workspaceId": entry.get("workspace_id").or_else(|| entry.get("wid")).cloned().unwrap_or(Value::Null),
            "tags": entry.get("tags").cloned().unwrap_or_else(|| json!([])),
            "source": "imported",
            "provider": "toggl",
            "externalId": external_id,
        },
        "createdAt": started_at,
        "updatedAt": updated_at,
        "deletedAt": null,
    }))
}

pub(crate) fn toggl_before_cursor(entries: &[Value]) -> Option<String> {
    entries
        .iter()
        .filter_map(|entry| entry.get("start").and_then(Value::as_str))
        .filter_map(|value| DateTime::parse_from_rfc3339(value).ok())
        .min()
        .map(|value| (value - ChronoDuration::milliseconds(1)).to_rfc3339())
}

pub(crate) async fn fetch_toggl_entries(
    client: &reqwest::Client,
    secret: &str,
    since: Option<i64>,
) -> Result<Vec<Value>, String> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut before: Option<String> = None;

    loop {
        let mut request = authenticated_get(
            client,
            Provider::Toggl,
            format!("{TOGGL_BASE_URL}/me/time_entries"),
            secret,
        );
        if let Some(value) = since {
            request = request.query(&[("since", value.to_string())]);
        }
        if let Some(value) = before.as_ref() {
            request = request.query(&[("before", value)]);
        }

        let response = request
            .send()
            .await
            .map_err(|error| format!("Toggl Track: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Toggl Track РІРµСЂРЅСѓР» HTTP {}",
                response.status()
            ));
        }
        let page: Vec<Value> = response
            .json()
            .await
            .map_err(|error| format!("РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ РѕС‚РІРµС‚ Toggl Track: {error}"))?;
        if page.is_empty() {
            break;
        }

        let Some(next_before) = toggl_before_cursor(&page) else {
            break;
        };
        for entry in page {
            if entry
                .get("id")
                .and_then(Value::as_i64)
                .is_none_or(|id| seen.insert(id))
            {
                entries.push(entry);
            }
        }
        if before
            .as_deref()
            .is_some_and(|value| value <= next_before.as_str())
        {
            break;
        }
        before = Some(next_before);
    }

    Ok(entries)
}

pub(crate) async fn sync_toggl(
    ark: &ArkHost,
    secret: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let client = http_client()?;
    let since = settings
        .last_success_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp());
    let entries = fetch_toggl_entries(&client, secret, since).await?;

    ensure_object_type(
        ark,
        TIME_ENTRY_TYPE_ID,
        "Р—Р°РїРёСЃСЊ РІСЂРµРјРµРЅРё",
        started_at,
    )
    .await?;
    let mut imported = 0_u64;
    for entry in entries {
        let external_id = entry.get("id").and_then(Value::as_i64);
        let deleted = entry
            .get("server_deleted_at")
            .or_else(|| entry.get("deleted_at"))
            .is_some_and(|value| !value.is_null());
        if deleted {
            if let Some(id) = external_id {
                ark_request(
                    ark,
                    "delete_object",
                    json!({ "id": format!("toggl-time-entry:{id}") }),
                )
                .await?;
            }
            continue;
        }
        ark_request(
            ark,
            "upsert_object",
            json!({ "object": toggl_time_entry_object(&entry)? }),
        )
        .await?;
        imported += 1;
    }
    Ok(imported)
}
