use super::*;

pub async fn handle_operation(
    subop: &str,
    params: Value,
    ark: &ArkHost,
    data_dir: &Path,
) -> Result<Value, String> {
    match subop {
        "list" => Ok(snapshot(&read_config(data_dir))),
        "update_settings" => {
            let provider = provider_from_params(&params)?;
            let interval = params
                .get("intervalMinutes")
                .and_then(Value::as_u64)
                .map(validate_interval)
                .transpose()?;
            let startup = params.get("syncOnStartup").and_then(Value::as_bool);
            let config = mutate_config(data_dir, |config| {
                let settings = config.provider_mut(provider);
                if let Some(value) = interval {
                    settings.interval_minutes = value;
                }
                if let Some(value) = startup {
                    settings.sync_on_startup = value;
                }
                Ok(())
            })?;
            Ok(snapshot(&config))
        }
        "set_credential" => {
            let provider = provider_from_params(&params)?;
            let secret = params
                .get("credential")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("Укажите: {}", provider.credential_label()))?;
            let max_len = if provider == Provider::Greatfrontend {
                16_384
            } else {
                2_048
            };
            if secret.len() > max_len {
                return Err("Данные подключения слишком длинные".to_string());
            }
            verify_credential(provider, secret).await?;
            save_credential(provider, secret)?;
            let config = mutate_config(data_dir, |config| {
                let settings = config.provider_mut(provider);
                settings.last_attempt_at = None;
                settings.last_success_at = None;
                settings.last_error = None;
                settings.imported_count = 0;
                Ok(())
            })?;
            Ok(snapshot(&config))
        }
        "clear_credential" => {
            delete_credential(provider_from_params(&params)?)?;
            Ok(snapshot(&read_config(data_dir)))
        }
        "sync_now" => sync_provider(ark, data_dir, provider_from_params(&params)?).await,
        "body_weight_set" => {
            let body_weight = params.get("bodyWeightKg").and_then(Value::as_f64);
            if body_weight.is_some_and(|value| !(20.0..=400.0).contains(&value)) {
                return Err(
                    "Р’РµСЃ С‚РµР»Р° РґРѕР»Р¶РµРЅ Р±С‹С‚СЊ РѕС‚ 20 РґРѕ 400 РєРі".to_string(),
                );
            }
            let config = mutate_config(data_dir, |config| {
                config.body_weight_kg = body_weight;
                Ok(())
            })?;
            Ok(snapshot(&config))
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
        _ => Err(format!(
            "РќРµРёР·РІРµСЃС‚РЅР°СЏ РѕРїРµСЂР°С†РёСЏ integrations.{subop}"
        )),
    }
}
