use super::*;

pub(crate) fn leetcode_submission_timestamp(submission: &Value) -> Option<DateTime<Utc>> {
    submission
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .and_then(|value| DateTime::<Utc>::from_timestamp(value, 0))
}

pub(crate) fn leetcode_submission_is_new_enough(
    submission: &Value,
    cutoff: Option<DateTime<Utc>>,
) -> bool {
    cutoff.is_none_or(|cutoff| {
        leetcode_submission_timestamp(submission).is_none_or(|value| value >= cutoff)
    })
}

pub(crate) fn leetcode_page_reached_cutoff(items: &[Value], cutoff: Option<DateTime<Utc>>) -> bool {
    cutoff.is_some_and(|cutoff| {
        items.iter().any(|submission| {
            leetcode_submission_timestamp(submission).is_some_and(|value| value < cutoff)
        })
    })
}

pub(crate) fn leetcode_submission_object(
    submission: &Value,
    question_numbers: &HashMap<String, String>,
) -> Result<Value, String> {
    let external_id = submission
        .get("id")
        .and_then(Value::as_str)
        .ok_or("LeetCode submission Р±РµР· id")?;
    let title = submission
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("LeetCode");
    let status = submission
        .get("statusDisplay")
        .and_then(Value::as_str)
        .unwrap_or("Unknown");
    let slug = submission
        .get("titleSlug")
        .and_then(Value::as_str)
        .unwrap_or("");
    let timestamp = leetcode_submission_timestamp(submission)
        .map(|value| value.to_rfc3339())
        .ok_or("LeetCode submission Р±РµР· РєРѕСЂСЂРµРєС‚РЅРѕР№ РґР°С‚С‹")?;
    let raw_url = submission.get("url").and_then(Value::as_str).unwrap_or("");
    let url = if raw_url.starts_with("http") {
        raw_url.to_string()
    } else {
        format!("https://leetcode.com{raw_url}")
    };
    Ok(json!({
        "id": format!("leetcode-submission:{external_id}"),
        "typeId": CODING_SUBMISSION_TYPE_ID,
        "title": format!("{title} вЂ” {status}"),
        "contentJson": {},
        "propsJson": {
            "source": "leetcode",
            "externalId": external_id,
            "problemTitle": title,
            "problemSlug": slug,
            "problemNumber": question_numbers.get(slug).cloned().unwrap_or_default(),
            "status": status,
            "accepted": status == "Accepted",
            "language": submission.get("lang").cloned().unwrap_or(Value::Null),
            "runtime": submission.get("runtime").cloned().unwrap_or(Value::Null),
            "memory": submission.get("memory").cloned().unwrap_or(Value::Null),
            "submittedAt": timestamp,
            "url": url,
        },
        "createdAt": timestamp,
        "updatedAt": timestamp,
        "deletedAt": null,
    }))
}

pub(crate) fn leetcode_question_numbers_query(slugs: &[String]) -> Value {
    let declarations = slugs
        .iter()
        .enumerate()
        .map(|(index, _)| format!("$slug{index}: String!"))
        .collect::<Vec<_>>()
        .join(", ");
    let fields = slugs
        .iter()
        .enumerate()
        .map(|(index, _)| {
            format!("q{index}: question(titleSlug: $slug{index}) {{ questionFrontendId }}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let variables = slugs
        .iter()
        .enumerate()
        .map(|(index, slug)| (format!("slug{index}"), json!(slug)))
        .collect::<serde_json::Map<_, _>>();
    json!({
        "query": format!("query questionNumbers({declarations}) {{ {fields} }}"),
        "variables": variables,
    })
}
