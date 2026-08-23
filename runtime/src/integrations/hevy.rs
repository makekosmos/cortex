use super::*;

pub(crate) async fn fetch_hevy_pages(
    client: &reqwest::Client,
    secret: &str,
    path: &str,
    item_key: &str,
    page_size: u32,
    extra_query: &[(&str, String)],
) -> Result<Vec<Value>, String> {
    let mut page = 1_u32;
    let mut items = Vec::new();
    loop {
        let mut request = authenticated_get(
            client,
            Provider::Hevy,
            format!("{HEVY_BASE_URL}{path}"),
            secret,
        )
        .query(&[
            ("page", page.to_string()),
            ("pageSize", page_size.to_string()),
        ]);
        if !extra_query.is_empty() {
            request = request.query(extra_query);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("Hevy: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Hevy РІРµСЂРЅСѓР» HTTP {}", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|error| format!("РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ РѕС‚РІРµС‚ Hevy: {error}"))?;
        items.extend(
            body.get(item_key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        );
        let page_count = body.get("page_count").and_then(Value::as_u64).unwrap_or(1);
        if u64::from(page) >= page_count || page >= 10_000 {
            break;
        }
        page += 1;
    }
    Ok(items)
}

pub(crate) fn annotate_hevy_workout(workout: &mut Value, templates: &HashMap<String, Value>) {
    let Some(exercises) = workout.get_mut("exercises").and_then(Value::as_array_mut) else {
        return;
    };
    for exercise in exercises {
        let Some(object) = exercise.as_object_mut() else {
            continue;
        };
        let Some(template) = object
            .get("exercise_template_id")
            .and_then(Value::as_str)
            .and_then(|id| templates.get(id))
        else {
            continue;
        };
        for (source, target) in [
            ("primary_muscle_group", "primaryMuscleGroup"),
            ("secondary_muscle_groups", "secondaryMuscleGroups"),
            ("equipment_category", "equipmentCategory"),
            ("type", "exerciseType"),
        ] {
            if let Some(value) = template.get(source) {
                object.insert(target.to_string(), value.clone());
            }
        }
    }
}

pub(crate) fn hevy_workout_object(workout: &Value) -> Result<Value, String> {
    let external_id = workout
        .get("id")
        .and_then(Value::as_str)
        .ok_or("Hevy workout Р±РµР· id")?;
    let title = workout
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("РўСЂРµРЅРёСЂРѕРІРєР°");
    let created_at = workout
        .get("created_at")
        .or_else(|| workout.get("start_time"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z");
    let updated_at = workout
        .get("updated_at")
        .and_then(Value::as_str)
        .unwrap_or(created_at);
    Ok(json!({
        "id": format!("hevy-workout:{external_id}"),
        "typeId": WORKOUT_TYPE_ID,
        "title": title,
        "contentJson": { "description": workout.get("description").cloned().unwrap_or(Value::Null) },
        "propsJson": {
            "source": "hevy",
            "externalId": external_id,
            "routineId": workout.get("routine_id").cloned().unwrap_or(Value::Null),
            "startedAt": workout.get("start_time").cloned().unwrap_or(Value::Null),
            "endedAt": workout.get("end_time").cloned().unwrap_or(Value::Null),
            "providerUpdatedAt": workout.get("updated_at").cloned().unwrap_or(Value::Null),
            "exercises": workout.get("exercises").cloned().unwrap_or_else(|| json!([])),
        },
        "createdAt": created_at,
        "updatedAt": updated_at,
        "deletedAt": null,
    }))
}

pub(crate) async fn sync_hevy(
    ark: &ArkHost,
    secret: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let client = http_client()?;
    let templates = fetch_hevy_pages(
        &client,
        secret,
        "/v1/exercise_templates",
        "exercise_templates",
        100,
        &[],
    )
    .await?
    .into_iter()
    .filter_map(|template| {
        let id = template.get("id")?.as_str()?.to_string();
        Some((id, template))
    })
    .collect::<HashMap<_, _>>();

    let mut deleted_ids = Vec::new();
    let workouts = if let Some(since) = settings.last_success_at.as_ref() {
        let events = fetch_hevy_pages(
            &client,
            secret,
            "/v1/workouts/events",
            "events",
            10,
            &[("since", since.clone())],
        )
        .await?;
        let mut updated = Vec::new();
        for event in events {
            match event.get("type").and_then(Value::as_str) {
                Some("updated") => {
                    if let Some(workout) = event.get("workout") {
                        updated.push(workout.clone());
                    }
                }
                Some("deleted") => {
                    if let Some(id) = event.get("id").and_then(Value::as_str) {
                        deleted_ids.push(id.to_string());
                    }
                }
                _ => {}
            }
        }
        updated
    } else {
        fetch_hevy_pages(&client, secret, "/v1/workouts", "workouts", 10, &[]).await?
    };

    ensure_object_type(ark, WORKOUT_TYPE_ID, "РўСЂРµРЅРёСЂРѕРІРєР°", started_at).await?;
    let mut imported = 0_u64;
    for mut workout in workouts {
        annotate_hevy_workout(&mut workout, &templates);
        ark_request(
            ark,
            "upsert_object",
            json!({ "object": hevy_workout_object(&workout)? }),
        )
        .await?;
        imported += 1;
    }
    for external_id in deleted_ids {
        ark_request(
            ark,
            "delete_object",
            json!({ "id": format!("hevy-workout:{external_id}") }),
        )
        .await?;
    }
    Ok(imported)
}
