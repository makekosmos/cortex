use super::*;

#[test]
fn hevy_mapping_and_body_metrics_are_stable() {
    let mut workout = json!({
        "id": "workout-1",
        "title": "Push",
        "start_time": Utc::now().to_rfc3339(),
        "end_time": Utc::now().to_rfc3339(),
        "created_at": Utc::now().to_rfc3339(),
        "updated_at": Utc::now().to_rfc3339(),
        "exercises": [{
            "title": "Bench Press (Barbell)",
            "exercise_template_id": "bench",
            "sets": [{ "type": "normal", "weight_kg": 100.0, "reps": 5.0 }]
        }]
    });
    annotate_hevy_workout(
        &mut workout,
        &HashMap::from([(
            "bench".to_string(),
            json!({
                "primary_muscle_group": "chest",
                "secondary_muscle_groups": ["triceps"],
                "equipment_category": "barbell",
                "type": "weight_reps"
            }),
        )]),
    );
    let object = hevy_workout_object(&workout).expect("map workout");
    assert_eq!(object["id"], "hevy-workout:workout-1");
    assert_eq!(object["typeId"], WORKOUT_TYPE_ID);
    let metrics = body_metrics(&[object], Some(80.0), 7);
    let development = metrics["development"].as_array().expect("development");
    assert!(development.iter().any(|row| row["muscle"] == "chest"));
    assert!(development.iter().any(|row| row["muscle"] == "triceps"));
    let load = metrics["load"].as_array().expect("load");
    let chest = load
        .iter()
        .find(|row| row["muscle"] == "chest")
        .expect("chest");
    let triceps = load
        .iter()
        .find(|row| row["muscle"] == "triceps")
        .expect("triceps");
    assert_eq!(chest["tonnageKg"], 500.0);
    assert_eq!(triceps["tonnageKg"], 250.0);
}

#[test]
fn toggl_mapping_preserves_running_entry() {
    let entry = toggl_time_entry_object(&json!({
        "id": 42,
        "description": "РџСЂРѕРµРєС‚",
        "start": "2026-07-17T10:00:00Z",
        "stop": null,
        "duration": -1,
        "at": "2026-07-17T10:00:00Z",
        "workspace_id": 7,
        "tags": ["deep-work"]
    }))
    .expect("map time entry");
    assert_eq!(entry["id"], "toggl-time-entry:42");
    assert_eq!(entry["propsJson"]["endedAt"], Value::Null);
    assert_eq!(entry["propsJson"]["source"], "imported");
}

#[test]
fn leetcode_mapping_keeps_attempt_metadata_without_code() {
    let submission = leetcode_submission_object(
        &json!({
            "id": "1972542025",
            "title": "Contains Duplicate",
            "titleSlug": "contains-duplicate",
            "statusDisplay": "Wrong Answer",
            "lang": "javascript",
            "timestamp": "1775606400",
            "url": "/submissions/detail/1972542025/",
            "runtime": "N/A",
            "memory": "N/A"
        }),
        &HashMap::from([("contains-duplicate".to_string(), "217".to_string())]),
    )
    .expect("map submission");
    assert_eq!(submission["id"], "leetcode-submission:1972542025");
    assert_eq!(submission["propsJson"]["status"], "Wrong Answer");
    assert_eq!(submission["propsJson"]["accepted"], false);
    assert_eq!(submission["propsJson"]["problemNumber"], "217");
    assert!(submission["propsJson"].get("code").is_none());
}

#[test]
fn leetcode_question_number_query_batches_slugs() {
    let query =
        leetcode_question_numbers_query(&["two-sum".to_string(), "contains-duplicate".to_string()]);
    assert_eq!(query["variables"]["slug0"], "two-sum");
    assert_eq!(query["variables"]["slug1"], "contains-duplicate");
    assert!(query["query"]
        .as_str()
        .is_some_and(|value| value.contains("q1: question(titleSlug: $slug1)")));
}

#[test]
fn leetcode_question_number_backfill_reads_ark_object_shape() {
    assert!(leetcode_objects_need_question_number_backfill(&json!([{
        "deletedAt": null,
        "propsJson": {
            "source": "leetcode",
            "problemTitle": "Two Sum"
        }
    }])));
    assert!(!leetcode_objects_need_question_number_backfill(&json!([{
        "deleted_at": null,
        "props_json": {
            "source": "leetcode",
            "problemNumber": "1"
        }
    }])));
}

#[test]
fn leetcode_profile_maps_homepage_difficulty_counts() {
    let profile = leetcode_profile_object(
        "tester",
        &json!({
            "data": {
                "matchedUser": { "submitStatsGlobal": { "acSubmissionNum": [
                    { "difficulty": "All", "count": 78 },
                    { "difficulty": "Easy", "count": 61 },
                    { "difficulty": "Medium", "count": 17 },
                    { "difficulty": "Hard", "count": 0 }
                ]}},
                "allQuestionsCount": [
                    { "difficulty": "All", "count": 3991 },
                    { "difficulty": "Easy", "count": 954 },
                    { "difficulty": "Medium", "count": 2084 },
                    { "difficulty": "Hard", "count": 953 }
                ]
            }
        }),
        "2026-07-18T00:00:00Z",
    )
    .expect("profile");
    assert_eq!(profile["propsJson"]["solved"]["all"], 78);
    assert_eq!(profile["propsJson"]["solved"]["easy"], 61);
    assert_eq!(profile["propsJson"]["available"]["hard"], 953);
}

#[test]
fn leetcode_incremental_page_stops_at_overlap_cutoff() {
    // Regression: 2026-07-17. Every sync used to walk the complete submission history.
    let cutoff = DateTime::<Utc>::from_timestamp(150, 0).expect("cutoff");
    let recent = json!({ "timestamp": "200" });
    let old = json!({ "timestamp": "100" });
    let page = vec![recent.clone(), old.clone()];

    assert!(leetcode_page_reached_cutoff(&page, Some(cutoff)));
    assert!(leetcode_submission_is_new_enough(&recent, Some(cutoff)));
    assert!(!leetcode_submission_is_new_enough(&old, Some(cutoff)));
    assert!(!leetcode_page_reached_cutoff(&page, None));
}
