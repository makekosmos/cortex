use super::integration_codewars::{
    codewars_completion_is_new_enough, codewars_completion_object,
    codewars_objects_need_rank_backfill, codewars_page_reached_cutoff, codewars_profile_object,
};
use super::*;

#[test]
fn codewars_mapping_keeps_completed_kata_without_solution_code() {
    let completion = codewars_completion_object(
        "Tester",
        &json!({
            "id": "514b92a657cdc65150000006",
            "name": "Multiples of 3 and 5",
            "slug": "multiples-of-3-and-5",
            "completedAt": "2017-04-06T16:32:09Z",
            "completedLanguages": ["javascript", "ruby", "javascript"]
        }),
        &json!({ "id": -6, "name": "6 kyu", "color": "yellow" }),
    )
    .expect("map completion");
    assert_eq!(
        completion["id"],
        "codewars-completion:tester:514b92a657cdc65150000006"
    );
    assert_eq!(completion["propsJson"]["username"], "Tester");
    assert_eq!(completion["propsJson"]["status"], "Completed");
    assert_eq!(completion["propsJson"]["rank"]["name"], "6 kyu");
    assert_eq!(
        completion["propsJson"]["languages"],
        json!(["javascript", "ruby"])
    );
    assert_eq!(
        completion["propsJson"]["url"],
        "https://www.codewars.com/kata/multiples-of-3-and-5"
    );
    assert!(completion["propsJson"].get("code").is_none());
}

#[test]
fn codewars_rank_backfill_reads_ark_object_shape() {
    assert!(codewars_objects_need_rank_backfill(
        &json!([{
            "deletedAt": null,
            "propsJson": {
                "source": "codewars",
                "username": "Tester",
                "problemTitle": "Multiples of 3 and 5"
            }
        }]),
        "tester"
    ));
    assert!(!codewars_objects_need_rank_backfill(
        &json!([{
            "deleted_at": null,
            "props_json": {
                "source": "codewars",
                "username": "Tester",
                "rank": null
            }
        }]),
        "tester"
    ));
}

#[test]
fn codewars_profile_maps_public_rank_and_totals() {
    let profile = codewars_profile_object(
        &json!({
            "username": "tester",
            "honor": 544,
            "leaderboardPosition": 134,
            "ranks": {
                "overall": { "rank": -3, "name": "3 kyu", "color": "blue", "score": 2116 },
                "languages": { "javascript": { "rank": -3, "name": "3 kyu", "score": 1819 } }
            },
            "codeChallenges": { "totalCompleted": 230 }
        }),
        "2026-07-18T00:00:00Z",
    )
    .expect("profile");
    assert_eq!(profile["propsJson"]["solved"]["all"], 230);
    assert_eq!(profile["propsJson"]["rank"]["name"], "3 kyu");
    assert_eq!(profile["propsJson"]["honor"], 544);
}

#[test]
fn codewars_incremental_page_stops_only_when_page_is_old() {
    let cutoff = DateTime::parse_from_rfc3339("2026-01-02T00:00:00Z")
        .expect("cutoff")
        .with_timezone(&Utc);
    let recent = json!({ "completedAt": "2026-01-03T00:00:00Z" });
    let old = json!({ "completedAt": "2026-01-01T00:00:00Z" });
    assert!(!codewars_page_reached_cutoff(
        &[recent.clone(), old.clone()],
        Some(cutoff)
    ));
    assert!(codewars_page_reached_cutoff(
        &[old.clone(), old],
        Some(cutoff)
    ));
    assert!(codewars_completion_is_new_enough(&recent, Some(cutoff)));
}

#[test]
fn duplicate_provider_sync_does_not_wait_for_the_first_one() {
    // Regression: 2026-07-17. Manual sync queued behind startup sync and then repeated it.
    let lock = sync_lock(Provider::Leetcode.sync_lock_index());
    let first = lock.try_lock().expect("first sync owns lock");
    assert!(lock.try_lock().is_err());
    drop(first);
    assert!(lock.try_lock().is_ok());
}

#[test]
fn provider_requests_follow_official_auth_contracts() {
    let client = http_client().expect("client");
    let hevy = authenticated_get(
        &client,
        Provider::Hevy,
        "https://api.hevyapp.com/v1/workouts".to_string(),
        "test-hevy-key",
    )
    .build()
    .expect("hevy request");
    assert_eq!(hevy.url().path(), "/v1/workouts");
    assert_eq!(hevy.headers()["api-key"], "test-hevy-key");
    assert!(hevy.headers().get("authorization").is_none());

    let toggl = authenticated_get(
        &client,
        Provider::Toggl,
        "https://api.track.toggl.com/api/v9/me/time_entries".to_string(),
        "test-token",
    )
    .build()
    .expect("toggl request");
    assert_eq!(toggl.url().path(), "/api/v9/me/time_entries");
    assert_eq!(
        toggl.headers()["authorization"],
        "Basic dGVzdC10b2tlbjphcGlfdG9rZW4="
    );
}

#[test]
fn scheduler_interval_is_bounded() {
    assert_eq!(validate_interval(60), Ok(60));
    assert!(validate_interval(1).is_err());
}

#[test]
fn provider_parameters_are_trimmed_and_case_insensitive() {
    assert_eq!(
        provider_from_params(&json!({ "provider": "  LeEtCoDe  " })),
        Ok(Provider::Leetcode)
    );
    assert!(provider_from_params(&json!({})).is_err());
    assert!(provider_from_params(&json!({ "provider": "calendar" })).is_err());
}

#[test]
fn default_snapshot_exposes_all_providers_without_credentials() {
    let snapshot = snapshot(&IntegrationsConfig::default());
    let providers = snapshot["providers"].as_array().expect("providers");
    assert_eq!(providers.len(), 4);
    assert_eq!(providers[0]["id"], "hevy");
    assert_eq!(providers[1]["id"], "toggl");
    assert_eq!(providers[2]["id"], "leetcode");
    assert_eq!(providers[3]["id"], "codewars");
    assert!(providers.iter().all(|provider| {
        provider["hasCredential"] == false && provider["settings"]["importedCount"] == 0
    }));
}

#[test]
fn leetcode_auth_request_keeps_session_contract() {
    let client = http_client().expect("client");
    let request = leetcode_request(
        &client,
        &json!({ "session": "session-value", "csrfToken": "csrf-value" }).to_string(),
        json!({ "query": "query" }),
    )
    .expect("request")
    .build()
    .expect("built request");
    assert_eq!(request.url().as_str(), LEETCODE_GRAPHQL_URL);
    assert_eq!(request.headers()["x-csrftoken"], "csrf-value");
    assert_eq!(
        request.headers()[reqwest::header::COOKIE],
        "LEETCODE_SESSION=session-value; csrftoken=csrf-value"
    );
}

#[test]
fn toggl_pagination_moves_before_the_oldest_entry() {
    let cursor = toggl_before_cursor(&[
        json!({ "start": "2026-07-17T10:00:00Z" }),
        json!({ "start": "2020-01-01T00:00:00Z" }),
    ])
    .expect("cursor");
    assert_eq!(cursor, "2019-12-31T23:59:59.999+00:00");
}
