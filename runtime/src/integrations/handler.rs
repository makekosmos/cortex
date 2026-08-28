use super::*;

fn provider_id(params: &Value) -> Result<&str, String> {
    params
        .get("provider")
        .and_then(Value::as_str)
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        .ok_or_else(|| "Не указана интеграция".to_string())
}

fn snapshot(
    data_dir: &Path,
    packages: &crate::package_service::PackageService,
) -> Result<Value, String> {
    Ok(json!({
        "providers": packages
            .integration_provider_snapshots()
            .map_err(|_| "Не удалось прочитать пакетные интеграции".to_string())?,
        "bodyWeightKg": read_config(data_dir).body_weight_kg,
    }))
}

pub async fn handle_operation(
    subop: &str,
    params: Value,
    ark: &ArkHost,
    data_dir: &Path,
    packages: &crate::package_service::PackageService,
) -> Result<Value, String> {
    match subop {
        "list" => snapshot(data_dir, packages),
        "login_contract" => packages
            .integration_login_contract(provider_id(&params)?)
            .map_err(|_| "Интеграция не содержит доверенного сценария входа".to_string()),
        "set_credential" => {
            let id = provider_id(&params)?;
            let value = params
                .get("credential")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Укажите данные подключения".to_string())?;
            packages
                .set_integration_value(id, params.get("setting").and_then(Value::as_str), value)
                .await
                .map_err(|_| "Не удалось сохранить настройку интеграции".to_string())?;
            snapshot(data_dir, packages)
        }
        "clear_credential" => {
            packages
                .clear_integration_values(provider_id(&params)?)
                .await
                .map_err(|_| "Не удалось очистить настройки интеграции".to_string())?;
            snapshot(data_dir, packages)
        }
        "sync_now" => {
            packages
                .sync_integration_now(provider_id(&params)?)
                .map_err(|_| "Интеграция недоступна".to_string())?;
            snapshot(data_dir, packages)
        }
        "body_weight_set" => {
            let body_weight = params.get("bodyWeightKg").and_then(Value::as_f64);
            if body_weight.is_some_and(|value| !(20.0..=400.0).contains(&value)) {
                return Err("Вес тела должен быть от 20 до 400 кг".to_string());
            }
            mutate_config(data_dir, |config| {
                config.body_weight_kg = body_weight;
                Ok(())
            })?;
            snapshot(data_dir, packages)
        }
        "body_snapshot" => {
            body_snapshot(
                ark,
                data_dir,
                params
                    .get("range")
                    .and_then(Value::as_str)
                    .unwrap_or("week"),
            )
            .await
        }
        _ => Err(format!("Неизвестная операция integrations.{subop}")),
    }
}
