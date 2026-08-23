use super::*;

pub(crate) async fn fetch_leetcode_question_numbers(
    client: &reqwest::Client,
    secret: &str,
    submissions: &[Value],
) -> Result<HashMap<String, String>, String> {
    let mut slugs = submissions
        .iter()
        .filter_map(|submission| submission.get("titleSlug").and_then(Value::as_str))
        .filter(|slug| {
            slug.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    slugs.sort();
    slugs.dedup();

    let mut numbers = HashMap::new();
    for chunk in slugs.chunks(40) {
        let response = leetcode_request(client, secret, leetcode_question_numbers_query(chunk))?
            .send()
            .await
            .map_err(|error| format!("LeetCode: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("LeetCode РІРµСЂРЅСѓР» HTTP {}", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|error| format!("РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ РѕС‚РІРµС‚ LeetCode: {error}"))?;
        for (index, slug) in chunk.iter().enumerate() {
            if let Some(number) = body
                .pointer(&format!("/data/q{index}/questionFrontendId"))
                .and_then(Value::as_str)
            {
                numbers.insert(slug.clone(), number.to_string());
            }
        }
    }
    Ok(numbers)
}

pub(crate) fn leetcode_objects_need_question_number_backfill(objects: &Value) -> bool {
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
                        props.get("source").and_then(Value::as_str) == Some("leetcode")
                            && !props.contains_key("problemNumber")
                    })
        })
    })
}

pub(crate) async fn leetcode_needs_question_number_backfill(ark: &ArkHost) -> Result<bool, String> {
    let objects = ark_request(
        ark,
        "list_objects_by_type",
        json!({ "type_id": CODING_SUBMISSION_TYPE_ID }),
    )
    .await?;
    Ok(leetcode_objects_need_question_number_backfill(&objects))
}

pub(crate) fn leetcode_count(rows: &Value, difficulty: &str) -> u64 {
    rows.as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("difficulty").and_then(Value::as_str) == Some(difficulty))
        })
        .and_then(|row| row.get("count"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

pub(crate) fn leetcode_profile_object(
    username: &str,
    body: &Value,
    timestamp: &str,
) -> Result<Value, String> {
    let solved = body
        .pointer("/data/matchedUser/submitStatsGlobal/acSubmissionNum")
        .ok_or("LeetCode РЅРµ РІРµСЂРЅСѓР» СЃС‚Р°С‚РёСЃС‚РёРєСѓ СЂРµС€С‘РЅРЅС‹С… Р·Р°РґР°С‡")?;
    let available = body
        .pointer("/data/allQuestionsCount")
        .ok_or("LeetCode РЅРµ РІРµСЂРЅСѓР» РєРѕР»РёС‡РµСЃС‚РІРѕ Р·Р°РґР°С‡")?;
    Ok(json!({
        "id": "leetcode-profile:current",
        "typeId": CODING_PROFILE_TYPE_ID,
        "title": format!("LeetCode вЂ” {username}"),
        "contentJson": {},
        "propsJson": {
            "source": "leetcode",
            "username": username,
            "solved": {
                "all": leetcode_count(solved, "All"),
                "easy": leetcode_count(solved, "Easy"),
                "medium": leetcode_count(solved, "Medium"),
                "hard": leetcode_count(solved, "Hard"),
            },
            "available": {
                "all": leetcode_count(available, "All"),
                "easy": leetcode_count(available, "Easy"),
                "medium": leetcode_count(available, "Medium"),
                "hard": leetcode_count(available, "Hard"),
            },
        },
        "createdAt": timestamp,
        "updatedAt": timestamp,
        "deletedAt": null,
    }))
}

pub(crate) async fn fetch_leetcode_profile(
    client: &reqwest::Client,
    secret: &str,
    timestamp: &str,
) -> Result<Value, String> {
    let status: Value = leetcode_request(
        client,
        secret,
        json!({ "query": "query globalData { userStatus { isSignedIn username } }" }),
    )?
    .send()
    .await
    .map_err(|error| format!("LeetCode: {error}"))?
    .json()
    .await
    .map_err(|error| format!("РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ РѕС‚РІРµС‚ LeetCode: {error}"))?;
    let username = status
        .pointer("/data/userStatus/username")
        .and_then(Value::as_str)
        .filter(|username| !username.is_empty())
        .ok_or("LeetCode РЅРµ РІРµСЂРЅСѓР» РёРјСЏ РїРѕР»СЊР·РѕРІР°С‚РµР»СЏ вЂ” РІРѕР№РґРёС‚Рµ Р·Р°РЅРѕРІРѕ")?;
    let body: Value = leetcode_request(
        client,
        secret,
        json!({
            "query": "query userProgress($username: String!) { matchedUser(username: $username) { submitStatsGlobal { acSubmissionNum { difficulty count submissions } } } allQuestionsCount { difficulty count } }",
            "variables": { "username": username },
        }),
    )?
    .send()
    .await
    .map_err(|error| format!("LeetCode: {error}"))?
    .json()
    .await
    .map_err(|error| format!("РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ РѕС‚РІРµС‚ LeetCode: {error}"))?;
    leetcode_profile_object(username, &body, timestamp)
}

pub(crate) async fn fetch_leetcode_submissions(
    client: &reqwest::Client,
    secret: &str,
    cutoff: Option<DateTime<Utc>>,
) -> Result<Vec<Value>, String> {
    let mut submissions = Vec::new();
    let mut seen = HashSet::new();
    let mut offset = 0_u64;
    let mut last_key: Option<String> = None;
    loop {
        let response =
            leetcode_request(client, secret, leetcode_query(offset, last_key.as_deref()))?
                .send()
                .await
                .map_err(|error| format!("LeetCode: {error}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(
                "РЎРµСЃСЃРёСЏ LeetCode РёСЃС‚РµРєР»Р° вЂ” РІРѕР№РґРёС‚Рµ Р·Р°РЅРѕРІРѕ".to_string(),
            );
        }
        if !response.status().is_success() {
            return Err(format!("LeetCode РІРµСЂРЅСѓР» HTTP {}", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|error| format!("РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ РѕС‚РІРµС‚ LeetCode: {error}"))?;
        if let Some(error) = body
            .get("errors")
            .and_then(Value::as_array)
            .and_then(|errors| errors.first())
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
        {
            return Err(format!("LeetCode: {error}"));
        }
        let page = body
            .pointer("/data/submissionList")
            .ok_or("LeetCode РЅРµ РІРµСЂРЅСѓР» РёСЃС‚РѕСЂРёСЋ вЂ” РІРѕР№РґРёС‚Рµ Р·Р°РЅРѕРІРѕ")?;
        let items = page
            .get("submissions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let page_len = items.len() as u64;
        let reached_cutoff = leetcode_page_reached_cutoff(&items, cutoff);
        for submission in items {
            if !leetcode_submission_is_new_enough(&submission, cutoff) {
                continue;
            }
            if submission
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| seen.insert(id.to_string()))
            {
                submissions.push(submission);
            }
        }
        if !page
            .get("hasNext")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || reached_cutoff
            || page_len == 0
            || offset >= 10_000
        {
            break;
        }
        offset += page_len;
        last_key = page
            .get("lastKey")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    Ok(submissions)
}
