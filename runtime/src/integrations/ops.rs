use super::*;

pub(crate) async fn verify_credential_at(
    provider: Provider,
    secret: &str,
    base_url: &str,
) -> Result<(), String> {
    let client = http_client()?;
    let path = match provider {
        Provider::Hevy => "/v1/user/info",
        Provider::Toggl => "/me",
        Provider::Leetcode => {
            let response = leetcode_request(&client, secret, leetcode_query(0, None))?
                .send()
                .await
                .map_err(|error| format!("Не удалось подключиться к LeetCode: {error}"))?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED
                || response.status() == reqwest::StatusCode::FORBIDDEN
            {
                return Err("Сессия LeetCode истекла — войдите заново".to_string());
            }
            let body: Value = response
                .json()
                .await
                .map_err(|error| format!("Некорректный ответ LeetCode: {error}"))?;
            return body
                .pointer("/data/submissionList/submissions")
                .and_then(Value::as_array)
                .map(|_| ())
                .ok_or_else(|| "LeetCode не вернул историю — войдите заново".to_string());
        }
        Provider::Codewars => {
            let response = client
                .get(codewars_user_url(secret, None)?)
                .send()
                .await
                .map_err(|error| format!("Не удалось подключиться к Codewars: {error}"))?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Err("Пользователь Codewars не найден".to_string());
            }
            if !response.status().is_success() {
                return Err(format!("Codewars вернул HTTP {}", response.status()));
            }
            let body: Value = response
                .json()
                .await
                .map_err(|error| format!("Некорректный ответ Codewars: {error}"))?;
            return body
                .get("username")
                .and_then(Value::as_str)
                .filter(|username| !username.is_empty())
                .map(|_| ())
                .ok_or_else(|| "Codewars не вернул профиль пользователя".to_string());
        }
        Provider::Greatfrontend => return verify_greatfrontend(secret).await,
        Provider::Bigfrontend => return verify_bigfrontend(secret).await,
    };
    let request = authenticated_get(&client, provider, format!("{base_url}{path}"), secret);
    let response = request
        .send()
        .await
        .map_err(|error| format!("Не удалось подключиться к {}: {error}", provider.label()))?;
    if response.status().is_success() {
        Ok(())
    } else if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        Err("Ключ не принят сервисом".to_string())
    } else {
        Err(format!(
            "{} вернул HTTP {}",
            provider.label(),
            response.status()
        ))
    }
}

pub(crate) async fn verify_credential(provider: Provider, secret: &str) -> Result<(), String> {
    verify_credential_at(
        provider,
        secret,
        match provider {
            Provider::Hevy => HEVY_BASE_URL,
            Provider::Toggl => TOGGL_BASE_URL,
            Provider::Leetcode => LEETCODE_GRAPHQL_URL,
            Provider::Codewars => CODEWARS_BASE_URL,
            Provider::Greatfrontend => "https://www.greatfrontend.com",
            Provider::Bigfrontend => "https://bigfrontend.dev",
        },
    )
    .await
}

pub(crate) async fn ark_request(
    ark: &ArkHost,
    operation: &str,
    params: Value,
) -> Result<Value, String> {
    let response = ark
        .request(operation, params)
        .await
        .map_err(|error| format!("ARK {operation}: {error}"))?;
    if response.ok {
        Ok(response.data)
    } else {
        Err(format!(
            "ARK {operation}: {}",
            response
                .error
                .unwrap_or_else(|| "unknown error".to_string())
        ))
    }
}

pub(crate) async fn ensure_object_type(
    ark: &ArkHost,
    id: &str,
    name: &str,
    now: &str,
) -> Result<(), String> {
    ark_request(
        ark,
        "upsert_object_type",
        json!({
            "object_type": {
                "id": id,
                "name": name,
                "schemaJson": "{}",
                "uiSchemaJson": "{}",
                "createdAt": now,
                "updatedAt": now,
                "systemLocked": false,
            }
        }),
    )
    .await?;
    Ok(())
}
