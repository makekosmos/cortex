use super::*;
use futures_util::{stream, StreamExt, TryStreamExt};

fn codewars_completion_timestamp(completion: &Value) -> Option<DateTime<Utc>> {
    completion
        .get("completedAt")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

pub(super) fn codewars_completion_is_new_enough(
    completion: &Value,
    cutoff: Option<DateTime<Utc>>,
) -> bool {
    cutoff.is_none_or(|cutoff| {
        codewars_completion_timestamp(completion).is_none_or(|value| value >= cutoff)
    })
}

pub(super) fn codewars_page_reached_cutoff(items: &[Value], cutoff: Option<DateTime<Utc>>) -> bool {
    cutoff.is_some_and(|cutoff| {
        !items.is_empty()
            && items.iter().all(|completion| {
                codewars_completion_timestamp(completion).is_some_and(|value| value < cutoff)
            })
    })
}

pub(super) fn codewars_completion_object(
    username: &str,
    completion: &Value,
    rank: &Value,
) -> Result<Value, String> {
    let external_id = completion
        .get("id")
        .and_then(Value::as_str)
        .ok_or("Codewars kata без id")?;
    let title = completion
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Codewars kata");
    let slug = completion.get("slug").and_then(Value::as_str).unwrap_or("");
    let completed_at = codewars_completion_timestamp(completion)
        .map(|value| value.to_rfc3339())
        .ok_or("Codewars kata без корректной даты")?;
    let mut languages = completion
        .get("completedLanguages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    languages.sort();
    languages.dedup();
    let language = languages.first().cloned().unwrap_or_default();
    Ok(json!({
        "id": format!("codewars-completion:{}:{external_id}", username.to_ascii_lowercase()),
        "typeId": CODING_SUBMISSION_TYPE_ID,
        "title": format!("{title} — завершено"),
        "contentJson": {},
        "propsJson": {
            "source": "codewars",
            "username": username,
            "externalId": external_id,
            "problemTitle": title,
            "problemSlug": slug,
            "problemNumber": "",
            "status": "Completed",
            "accepted": true,
            "language": language,
            "languages": languages,
            "runtime": null,
            "memory": null,
            "rank": rank,
            "submittedAt": completed_at,
            "url": format!("https://www.codewars.com/kata/{slug}"),
        },
        "createdAt": completed_at,
        "updatedAt": completed_at,
        "deletedAt": null,
    }))
}

pub(super) fn codewars_objects_need_rank_backfill(objects: &Value, username: &str) -> bool {
    objects.as_array().is_some_and(|objects| {
        objects.iter().any(|object| {
            object
                .get("deletedAt")
                .or_else(|| object.get("deleted_at"))
                .is_none_or(Value::is_null)
                && object
                    .get("propsJson")
                    .or_else(|| object.get("props_json"))
                    .and_then(Value::as_object)
                    .is_some_and(|props| {
                        props.get("source").and_then(Value::as_str) == Some("codewars")
                            && props
                                .get("username")
                                .and_then(Value::as_str)
                                .is_some_and(|value| value.eq_ignore_ascii_case(username))
                            && !props.contains_key("rank")
                    })
        })
    })
}

async fn codewars_needs_rank_backfill(ark: &ArkHost, username: &str) -> Result<bool, String> {
    let objects = ark_request(
        ark,
        "list_objects_by_type",
        json!({ "type_id": CODING_SUBMISSION_TYPE_ID }),
    )
    .await?;
    Ok(codewars_objects_need_rank_backfill(&objects, username))
}

async fn fetch_codewars_challenge_rank(
    client: &reqwest::Client,
    challenge: &str,
) -> Result<Value, String> {
    let response = client
        .get(codewars_challenge_url(challenge)?)
        .send()
        .await
        .map_err(|error| format!("Codewars: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Value::Null);
    }
    if !response.status().is_success() {
        return Err(format!("Codewars вернул HTTP {}", response.status()));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|error| format!("Некорректный ответ Codewars: {error}"))?;
    Ok(body.get("rank").cloned().unwrap_or(Value::Null))
}

async fn fetch_codewars_completion_ranks(
    client: &reqwest::Client,
    completions: Vec<Value>,
) -> Result<Vec<(Value, Value)>, String> {
    stream::iter(completions)
        .map(|completion| async move {
            let challenge = completion
                .get("id")
                .and_then(Value::as_str)
                .ok_or("Codewars kata без id")?;
            let rank = fetch_codewars_challenge_rank(client, challenge).await?;
            Ok((completion, rank))
        })
        .buffer_unordered(8)
        .try_collect()
        .await
}

pub(super) fn codewars_profile_object(body: &Value, timestamp: &str) -> Result<Value, String> {
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("Codewars не вернул имя пользователя")?;
    Ok(json!({
        "id": "codewars-profile:current",
        "typeId": CODING_PROFILE_TYPE_ID,
        "title": format!("Codewars — {username}"),
        "contentJson": {},
        "propsJson": {
            "source": "codewars",
            "username": username,
            "honor": body.get("honor").cloned().unwrap_or(Value::Null),
            "leaderboardPosition": body.get("leaderboardPosition").cloned().unwrap_or(Value::Null),
            "rank": body.pointer("/ranks/overall").cloned().unwrap_or(Value::Null),
            "languageRanks": body.pointer("/ranks/languages").cloned().unwrap_or(Value::Null),
            "solved": {
                "all": body.pointer("/codeChallenges/totalCompleted").cloned().unwrap_or(Value::Null),
            },
        },
        "createdAt": timestamp,
        "updatedAt": timestamp,
        "deletedAt": null,
    }))
}

async fn fetch_codewars_profile(client: &reqwest::Client, username: &str) -> Result<Value, String> {
    let response = client
        .get(codewars_user_url(username, None)?)
        .send()
        .await
        .map_err(|error| format!("Codewars: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Пользователь Codewars не найден".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("Codewars вернул HTTP {}", response.status()));
    }
    response
        .json()
        .await
        .map_err(|error| format!("Некорректный ответ Codewars: {error}"))
}

async fn fetch_codewars_completions(
    client: &reqwest::Client,
    username: &str,
    cutoff: Option<DateTime<Utc>>,
) -> Result<Vec<Value>, String> {
    let mut page = 0_u64;
    let mut completions = Vec::new();
    loop {
        let response = client
            .get(codewars_user_url(username, Some(page))?)
            .send()
            .await
            .map_err(|error| format!("Codewars: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Codewars вернул HTTP {}", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|error| format!("Некорректный ответ Codewars: {error}"))?;
        let items = body
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let reached_cutoff = codewars_page_reached_cutoff(&items, cutoff);
        completions.extend(
            items
                .iter()
                .filter(|item| codewars_completion_is_new_enough(item, cutoff))
                .cloned(),
        );
        let total_pages = body.get("totalPages").and_then(Value::as_u64).unwrap_or(0);
        if items.is_empty() || reached_cutoff || page + 1 >= total_pages || page >= 10_000 {
            break;
        }
        page += 1;
    }
    Ok(completions)
}

pub(super) async fn sync_codewars(
    ark: &ArkHost,
    username: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let client = http_client()?;
    let profile = fetch_codewars_profile(&client, username).await?;
    let canonical_username = profile
        .get("username")
        .and_then(Value::as_str)
        .ok_or("Codewars не вернул имя пользователя")?;
    // ponytail: старые rank кешируем; полный проход нужен только для backfill, пока API не даёт bulk ranks.
    let cutoff = if codewars_needs_rank_backfill(ark, canonical_username).await? {
        None
    } else {
        settings
            .last_success_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc) - ChronoDuration::days(1))
    };
    let completions = fetch_codewars_completions(&client, canonical_username, cutoff).await?;
    let completions = fetch_codewars_completion_ranks(&client, completions).await?;
    ensure_object_type(
        ark,
        CODING_SUBMISSION_TYPE_ID,
        "Отправка задачи",
        started_at,
    )
    .await?;
    ensure_object_type(
        ark,
        CODING_PROFILE_TYPE_ID,
        "Профиль программиста",
        started_at,
    )
    .await?;
    ark_request(
        ark,
        "upsert_object",
        json!({ "object": codewars_profile_object(&profile, started_at)? }),
    )
    .await?;
    let mut imported = 0_u64;
    for (completion, rank) in completions {
        ark_request(
            ark,
            "upsert_object",
            json!({ "object": codewars_completion_object(canonical_username, &completion, &rank)? }),
        )
        .await?;
        imported += 1;
    }
    Ok(imported)
}
