use super::*;

const GREATFRONTEND_INPUT: &str = r#"{"0":{"json":null,"meta":{"values":["undefined"],"v":1}}}"#;

#[derive(Deserialize)]
struct CookieCredential {
    cookies: Vec<CookiePair>,
}

#[derive(Deserialize)]
struct CookiePair {
    name: String,
    value: String,
}

pub(super) fn bigfrontend_profile(body: &str) -> Result<Value, String> {
    let marker = r#"<script id="__NEXT_DATA__" type="application/json">"#;
    let json = body
        .split_once(marker)
        .and_then(|(_, tail)| tail.split_once("</script>"))
        .map(|(json, _)| json)
        .ok_or("BigFrontend не вернул профиль")?;
    serde_json::from_str(json).map_err(|error| format!("Некорректный профиль BigFrontend: {error}"))
}

pub(super) async fn fetch_bigfrontend_items(
    client: &reqwest::Client,
    username: &str,
) -> Result<Vec<Value>, String> {
    let mut profile_url = reqwest::Url::parse(&format!("{BIGFRONTEND_ORIGIN}/user/"))
        .map_err(|error| format!("Некорректный адрес BigFrontend: {error}"))?;
    profile_url
        .path_segments_mut()
        .map_err(|_| "Некорректный адрес BigFrontend".to_string())?
        .push(username);
    let response = client
        .get(profile_url)
        .send()
        .await
        .map_err(|error| format!("BigFrontend: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Пользователь BigFrontend не найден".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("BigFrontend вернул HTTP {}", response.status()));
    }
    let profile = bigfrontend_profile(
        &response
            .text()
            .await
            .map_err(|error| format!("Некорректный профиль BigFrontend: {error}"))?,
    )?;
    let user_id = value_id(&profile["props"]["pageProps"]["profile"]["id"])
        .ok_or("BigFrontend не вернул id пользователя")?;
    let mut activity_url = reqwest::Url::parse(&format!("{BIGFRONTEND_ORIGIN}/api/activity"))
        .map_err(|error| format!("Некорректный адрес BigFrontend: {error}"))?;
    activity_url
        .query_pairs_mut()
        .append_pair("type", "submission")
        .append_pair("userId", &user_id);
    let body: Value = client
        .get(activity_url)
        .send()
        .await
        .map_err(|error| format!("BigFrontend: {error}"))?
        .error_for_status()
        .map_err(|error| format!("BigFrontend: {error}"))?
        .json()
        .await
        .map_err(|error| format!("Некорректный ответ BigFrontend: {error}"))?;
    Ok(body["items"].as_array().cloned().unwrap_or_default())
}

pub(super) fn greatfrontend_cookie(secret: &str) -> Result<String, String> {
    let credential: CookieCredential = serde_json::from_str(secret)
        .map_err(|_| "Сессия GreatFrontEnd повреждена — войдите заново".to_string())?;
    let cookies = credential
        .cookies
        .into_iter()
        .filter(|cookie| {
            !cookie.name.is_empty()
                && !cookie.value.is_empty()
                && !cookie.name.contains([';', '\r', '\n'])
                && !cookie.value.contains([';', '\r', '\n'])
        })
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect::<Vec<_>>();
    (!cookies.is_empty())
        .then(|| cookies.join("; "))
        .ok_or_else(|| "Сессия GreatFrontEnd пуста — войдите заново".to_string())
}

pub(super) async fn fetch_greatfrontend_items(
    client: &reqwest::Client,
    secret: &str,
) -> Result<Vec<Value>, String> {
    let mut url = reqwest::Url::parse(&format!(
        "{GREATFRONTEND_ORIGIN}/api/trpc/questionProgress.getAllIncludingMetadata"
    ))
    .map_err(|error| format!("Некорректный адрес GreatFrontEnd: {error}"))?;
    url.query_pairs_mut()
        .append_pair("batch", "1")
        .append_pair("input", GREATFRONTEND_INPUT);
    let response = client
        .get(url)
        .header(reqwest::header::COOKIE, greatfrontend_cookie(secret)?)
        .send()
        .await
        .map_err(|error| format!("GreatFrontEnd: {error}"))?;
    if matches!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        return Err("Сессия GreatFrontEnd истекла — войдите заново".to_string());
    }
    let body: Value = response
        .error_for_status()
        .map_err(|error| format!("GreatFrontEnd: {error}"))?
        .json()
        .await
        .map_err(|error| format!("Некорректный ответ GreatFrontEnd: {error}"))?;
    body.pointer("/0/result/data/json")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "GreatFrontEnd не вернул прогресс — войдите заново".to_string())
}

pub(super) async fn verify_bigfrontend(username: &str) -> Result<(), String> {
    fetch_bigfrontend_items(&http_client()?, username)
        .await
        .map(|_| ())
}

pub(super) async fn verify_greatfrontend(secret: &str) -> Result<(), String> {
    fetch_greatfrontend_items(&http_client()?, secret)
        .await
        .map(|_| ())
}
