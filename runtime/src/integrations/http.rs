use super::*;

pub(crate) fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Kosmos/1 integrations")
        .build()
        .map_err(|error| format!("РќРµ СѓРґР°Р»РѕСЃСЊ СЃРѕР·РґР°С‚СЊ HTTP-РєР»РёРµРЅС‚: {error}"))
}

pub(crate) fn authenticated_get(
    client: &reqwest::Client,
    provider: Provider,
    url: String,
    secret: &str,
) -> reqwest::RequestBuilder {
    match provider {
        Provider::Hevy => client.get(url).header("api-key", secret),
        Provider::Toggl => client.get(url).basic_auth(secret, Some("api_token")),
        Provider::Leetcode => client.get(url),
        Provider::Codewars => client.get(url),
        Provider::Greatfrontend => client.get(url),
        Provider::Bigfrontend => client.get(url),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LeetcodeCredential {
    session: String,
    csrf_token: String,
}

pub(crate) fn leetcode_request(
    client: &reqwest::Client,
    secret: &str,
    body: Value,
) -> Result<reqwest::RequestBuilder, String> {
    let credential: LeetcodeCredential = serde_json::from_str(secret).map_err(|_| {
        "РЎРµСЃСЃРёСЏ LeetCode РїРѕРІСЂРµР¶РґРµРЅР° вЂ” РІРѕР№РґРёС‚Рµ Р·Р°РЅРѕРІРѕ".to_string()
    })?;
    Ok(client
        .post(LEETCODE_GRAPHQL_URL)
        .header(
            reqwest::header::COOKIE,
            format!(
                "LEETCODE_SESSION={}; csrftoken={}",
                credential.session, credential.csrf_token
            ),
        )
        .header("x-csrftoken", credential.csrf_token)
        .header(reqwest::header::ORIGIN, "https://leetcode.com")
        .header(reqwest::header::REFERER, "https://leetcode.com/progress/")
        .json(&body))
}

pub(crate) fn leetcode_query(offset: u64, last_key: Option<&str>) -> Value {
    json!({
        "query": "query submissionList($offset: Int!, $limit: Int!, $lastKey: String) { submissionList(offset: $offset, limit: $limit, lastKey: $lastKey) { lastKey hasNext submissions { id title titleSlug statusDisplay lang timestamp url isPending memory runtime } } }",
        "variables": {
            "offset": offset,
            "limit": 20,
            "lastKey": last_key,
        }
    })
}

pub(crate) fn codewars_user_url(
    username: &str,
    completed_page: Option<u64>,
) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(&format!("{CODEWARS_BASE_URL}/"))
        .map_err(|error| format!("РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ Р°РґСЂРµСЃ Codewars: {error}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ Р°РґСЂРµСЃ Codewars".to_string())?;
        segments.push("users").push(username);
        if completed_page.is_some() {
            segments.push("code-challenges").push("completed");
        }
    }
    if let Some(page) = completed_page {
        url.query_pairs_mut().append_pair("page", &page.to_string());
    }
    Ok(url)
}

pub(crate) fn codewars_challenge_url(challenge: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(&format!("{CODEWARS_BASE_URL}/"))
        .map_err(|error| format!("РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ Р°РґСЂРµСЃ Codewars: {error}"))?;
    url.path_segments_mut()
        .map_err(|_| "РќРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ Р°РґСЂРµСЃ Codewars".to_string())?
        .push("code-challenges")
        .push(challenge);
    Ok(url)
}
