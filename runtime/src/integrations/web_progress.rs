use super::*;

pub(super) const BIGFRONTEND_ORIGIN: &str = "https://bigfrontend.dev";
pub(super) const GREATFRONTEND_ORIGIN: &str = "https://www.greatfrontend.com";

pub(super) fn value_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .or_else(|| value.as_u64().map(|value| value.to_string()))
}

fn timestamp(value: &Value) -> Option<DateTime<Utc>> {
    value
        .as_str()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
        .or_else(|| {
            value.as_i64().and_then(|value| {
                let seconds = if value > 10_000_000_000 {
                    value / 1_000
                } else {
                    value
                };
                DateTime::from_timestamp(seconds, 0)
            })
        })
}

fn cutoff(settings: &ProviderSettings) -> Option<DateTime<Utc>> {
    settings
        .last_success_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc) - ChronoDuration::days(1))
}

fn completion_object(
    source: &str,
    username: Option<&str>,
    external_id: &str,
    title: &str,
    slug: &str,
    format: &str,
    completed_at: DateTime<Utc>,
    url: String,
) -> Value {
    let completed_at = completed_at.to_rfc3339();
    json!({
        "id": format!("{source}-completion:{external_id}"),
        "typeId": CODING_SUBMISSION_TYPE_ID,
        "title": format!("{title} — завершено"),
        "contentJson": {},
        "propsJson": {
            "source": source,
            "username": username,
            "externalId": external_id,
            "problemTitle": title,
            "problemSlug": slug,
            "format": format,
            "status": "Completed",
            "accepted": true,
            "submittedAt": completed_at,
            "url": url,
        },
        "createdAt": completed_at,
        "updatedAt": completed_at,
        "deletedAt": null,
    })
}

pub(super) fn bigfrontend_completion_object(item: &Value) -> Result<Value, String> {
    let external_id = value_id(&item["id"]).ok_or("BigFrontend submission без id")?;
    let title = item
        .pointer("/target/title")
        .and_then(Value::as_str)
        .ok_or("BigFrontend submission без названия")?;
    let slug = item
        .pointer("/target/permalink")
        .and_then(Value::as_str)
        .ok_or("BigFrontend submission без permalink")?;
    let username = item.pointer("/user/username").and_then(Value::as_str);
    let completed_at =
        timestamp(&item["createdAt"]).ok_or("BigFrontend submission без корректной даты")?;
    Ok(completion_object(
        "bigfrontend",
        username,
        &external_id,
        title,
        slug,
        item.pointer("/target/targetType")
            .and_then(Value::as_str)
            .unwrap_or("problem"),
        completed_at,
        format!("{BIGFRONTEND_ORIGIN}/problem/{slug}"),
    ))
}

pub(super) fn greatfrontend_completion_object(item: &Value) -> Result<Value, String> {
    let external_id = value_id(&item["id"]).ok_or("GreatFrontEnd progress без id")?;
    let metadata = item
        .get("metadata")
        .ok_or("GreatFrontEnd progress без metadata")?;
    let title = metadata
        .get("title")
        .and_then(Value::as_str)
        .ok_or("GreatFrontEnd progress без названия")?;
    let slug = metadata
        .get("slug")
        .and_then(Value::as_str)
        .unwrap_or(&external_id);
    let href = metadata.get("href").and_then(Value::as_str).unwrap_or("");
    let url = if href.starts_with('/') {
        format!("{GREATFRONTEND_ORIGIN}{href}")
    } else {
        href.to_string()
    };
    let completed_at =
        timestamp(&item["createdAt"]).ok_or("GreatFrontEnd progress без корректной даты")?;
    Ok(completion_object(
        "greatfrontend",
        None,
        &external_id,
        title,
        slug,
        metadata
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("question"),
        completed_at,
        url,
    ))
}

async fn store_completions(
    ark: &ArkHost,
    items: Vec<Value>,
    cutoff: Option<DateTime<Utc>>,
    map: fn(&Value) -> Result<Value, String>,
    started_at: &str,
) -> Result<u64, String> {
    ensure_object_type(
        ark,
        CODING_SUBMISSION_TYPE_ID,
        "Отправка задачи",
        started_at,
    )
    .await?;
    let mut imported = 0;
    for item in items {
        if cutoff.is_some_and(|cutoff| timestamp(&item["createdAt"]).is_some_and(|at| at < cutoff))
        {
            continue;
        }
        ark_request(ark, "upsert_object", json!({ "object": map(&item)? })).await?;
        imported += 1;
    }
    Ok(imported)
}

pub(super) async fn sync_bigfrontend(
    ark: &ArkHost,
    username: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let items = fetch_bigfrontend_items(&http_client()?, username).await?;
    store_completions(
        ark,
        items,
        cutoff(settings),
        bigfrontend_completion_object,
        started_at,
    )
    .await
}

pub(super) async fn sync_greatfrontend(
    ark: &ArkHost,
    secret: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let items = fetch_greatfrontend_items(&http_client()?, secret).await?;
    store_completions(
        ark,
        items,
        cutoff(settings),
        greatfrontend_completion_object,
        started_at,
    )
    .await
}
