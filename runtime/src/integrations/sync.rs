use super::*;

pub(crate) async fn sync_leetcode(
    ark: &ArkHost,
    secret: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let client = http_client()?;
    let needs_number_backfill = leetcode_needs_question_number_backfill(ark).await?;
    let cutoff = if needs_number_backfill {
        None
    } else {
        settings
            .last_success_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc) - ChronoDuration::days(1))
    };
    let submissions = fetch_leetcode_submissions(&client, secret, cutoff).await?;
    let question_numbers = fetch_leetcode_question_numbers(&client, secret, &submissions).await?;
    let profile = fetch_leetcode_profile(&client, secret, started_at).await?;
    ensure_object_type(
        ark,
        CODING_SUBMISSION_TYPE_ID,
        "РћС‚РїСЂР°РІРєР° Р·Р°РґР°С‡Рё",
        started_at,
    )
    .await?;
    ensure_object_type(
        ark,
        CODING_PROFILE_TYPE_ID,
        "РџСЂРѕС„РёР»СЊ РїСЂРѕРіСЂР°РјРјРёСЃС‚Р°",
        started_at,
    )
    .await?;
    ark_request(ark, "upsert_object", json!({ "object": profile })).await?;
    let mut imported = 0_u64;
    for submission in submissions {
        ark_request(
            ark,
            "upsert_object",
            json!({ "object": leetcode_submission_object(&submission, &question_numbers)? }),
        )
        .await?;
        imported += 1;
    }
    Ok(imported)
}

pub(crate) async fn sync_provider(
    ark: &ArkHost,
    data_dir: &Path,
    provider: Provider,
) -> Result<Value, String> {
    let _guard = sync_lock(provider.sync_lock_index())
        .try_lock()
        .map_err(|_| {
            format!(
                "{} СѓР¶Рµ СЃРёРЅС…СЂРѕРЅРёР·РёСЂСѓРµС‚СЃСЏ",
                provider.label()
            )
        })?;
    let secret = read_credential(provider)
        .ok_or_else(|| "РЎРЅР°С‡Р°Р»Р° РїРѕРґРєР»СЋС‡РёС‚Рµ РёРЅС‚РµРіСЂР°С†РёСЋ".to_string())?;
    let started_at = Utc::now().to_rfc3339();
    let settings = mutate_config(data_dir, |config| {
        let settings = config.provider_mut(provider);
        settings.last_attempt_at = Some(started_at.clone());
        settings.last_error = None;
        Ok(())
    })?
    .provider(provider)
    .clone();

    let result = match provider {
        Provider::Hevy => sync_hevy(ark, &secret, &settings, &started_at).await,
        Provider::Toggl => sync_toggl(ark, &secret, &settings, &started_at).await,
        Provider::Leetcode => sync_leetcode(ark, &secret, &settings, &started_at).await,
        Provider::Codewars => {
            integration_codewars::sync_codewars(ark, &secret, &settings, &started_at).await
        }
    };
    match result {
        Ok(imported) => {
            let config = mutate_config(data_dir, |config| {
                let settings = config.provider_mut(provider);
                settings.last_success_at = Some(started_at.clone());
                settings.last_error = None;
                settings.imported_count = imported;
                Ok(())
            })?;
            Ok(json!({ "imported": imported, "snapshot": snapshot(&config) }))
        }
        Err(error) => {
            let message = error.to_string();
            let _ = mutate_config(data_dir, |config| {
                config.provider_mut(provider).last_error = Some(message.clone());
                Ok(())
            });
            Err(message)
        }
    }
}

pub(crate) fn is_due(settings: &ProviderSettings, now: DateTime<Utc>) -> bool {
    if settings.interval_minutes == 0 {
        return false;
    }
    settings
        .last_attempt_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|last| {
            now - last.with_timezone(&Utc)
                >= ChronoDuration::minutes(settings.interval_minutes as i64)
        })
        .unwrap_or(true)
}

pub fn spawn_scheduler(ark: Arc<ArkHost>, data_dir: PathBuf) {
    tokio::spawn(async move {
        let startup = read_config(&data_dir);
        for provider in [
            Provider::Hevy,
            Provider::Toggl,
            Provider::Leetcode,
            Provider::Codewars,
        ] {
            if startup.provider(provider).sync_on_startup && read_credential(provider).is_some() {
                if let Err(error) = sync_provider(&ark, &data_dir, provider).await {
                    tracing::warn!(provider = provider.id(), %error, "integration startup sync failed");
                }
            }
        }

        let mut timer = tokio::time::interval(Duration::from_secs(60));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        timer.tick().await;
        loop {
            timer.tick().await;
            let config = read_config(&data_dir);
            let now = Utc::now();
            for provider in [
                Provider::Hevy,
                Provider::Toggl,
                Provider::Leetcode,
                Provider::Codewars,
            ] {
                if read_credential(provider).is_some() && is_due(config.provider(provider), now) {
                    if let Err(error) = sync_provider(&ark, &data_dir, provider).await {
                        tracing::warn!(provider = provider.id(), %error, "integration scheduled sync failed");
                    }
                }
            }
        }
    });
}
