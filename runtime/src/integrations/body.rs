use super::*;

pub(crate) fn muscle_slug(value: &str) -> Option<&'static str> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .as_str()
    {
        "chest" => Some("chest"),
        "shoulders" | "front_deltoids" => Some("front-deltoids"),
        "rear_shoulders" | "back_deltoids" => Some("back-deltoids"),
        "biceps" => Some("biceps"),
        "triceps" => Some("triceps"),
        "forearms" | "forearm" => Some("forearm"),
        "upper_back" | "lats" => Some("upper-back"),
        "lower_back" => Some("lower-back"),
        "traps" | "trapezius" => Some("trapezius"),
        "abs" | "abdominals" => Some("abs"),
        "obliques" => Some("obliques"),
        "quadriceps" | "quads" => Some("quadriceps"),
        "hamstrings" | "hamstring" => Some("hamstring"),
        "glutes" | "gluteal" => Some("gluteal"),
        "calves" => Some("calves"),
        "adductors" | "adductor" => Some("adductor"),
        "abductors" | "abductor" => Some("abductors"),
        "neck" => Some("neck"),
        _ => None,
    }
}

pub(crate) fn benchmark_ratio(muscle: &str) -> f64 {
    match muscle {
        "chest" => 1.25,
        "front-deltoids" | "back-deltoids" => 0.75,
        "biceps" | "triceps" | "forearm" => 0.5,
        "upper-back" | "trapezius" => 1.25,
        "lower-back" | "hamstring" | "gluteal" => 1.75,
        "quadriceps" | "calves" => 1.5,
        "abs" | "obliques" | "adductor" | "abductors" | "neck" => 1.0,
        _ => 1.0,
    }
}

pub(crate) fn equipment_is_bodyweight(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "none" | "bodyweight"))
}

#[derive(Debug, Default, Clone)]
struct DevelopmentMetric {
    score: f64,
    best_e1rm_kg: f64,
    exercise: String,
}

pub(crate) fn body_metrics(
    objects: &[Value],
    body_weight_kg: Option<f64>,
    range_days: i64,
) -> Value {
    let now = Utc::now();
    let cutoff = now - ChronoDuration::days(range_days);
    let mut development: HashMap<String, DevelopmentMetric> = HashMap::new();
    let mut load: HashMap<String, f64> = HashMap::new();
    let mut workout_count = 0_u64;

    for object in objects {
        let props = object.get("propsJson").unwrap_or(&Value::Null);
        if props.get("source").and_then(Value::as_str) != Some("hevy") {
            continue;
        }
        let started = props
            .get("startedAt")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc));
        if started.is_some_and(|value| value >= cutoff) {
            workout_count += 1;
        }
        let Some(exercises) = props.get("exercises").and_then(Value::as_array) else {
            continue;
        };
        for exercise in exercises {
            let Some(primary) = exercise
                .get("primaryMuscleGroup")
                .and_then(Value::as_str)
                .and_then(muscle_slug)
            else {
                continue;
            };
            let secondary = exercise
                .get("secondaryMuscleGroups")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(muscle_slug)
                .collect::<Vec<_>>();
            let bodyweight = equipment_is_bodyweight(exercise.get("equipmentCategory"));
            let exercise_title = exercise
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("РЈРїСЂР°Р¶РЅРµРЅРёРµ");
            let Some(sets) = exercise.get("sets").and_then(Value::as_array) else {
                continue;
            };
            for set in sets {
                if set.get("type").and_then(Value::as_str) == Some("warmup") {
                    continue;
                }
                let reps = set.get("reps").and_then(Value::as_f64).unwrap_or(0.0);
                if reps <= 0.0 {
                    continue;
                }
                let added_weight = set.get("weight_kg").and_then(Value::as_f64).unwrap_or(0.0);
                let effective_weight = if bodyweight {
                    body_weight_kg.unwrap_or(0.0) + added_weight
                } else {
                    added_weight
                };
                if effective_weight <= 0.0 {
                    continue;
                }
                if let Some(weight) = body_weight_kg.filter(|weight| *weight > 0.0) {
                    let e1rm = effective_weight * (1.0 + reps.min(30.0) / 30.0);
                    let score = (e1rm / (weight * benchmark_ratio(primary))).clamp(0.0, 1.0);
                    let metric = development.entry(primary.to_string()).or_default();
                    if score > metric.score {
                        metric.score = score;
                        metric.best_e1rm_kg = e1rm;
                        metric.exercise = exercise_title.to_string();
                    }
                    for muscle in &secondary {
                        let metric = development.entry((*muscle).to_string()).or_default();
                        let secondary_score = score * 0.6;
                        if secondary_score > metric.score {
                            metric.score = secondary_score;
                            metric.best_e1rm_kg = e1rm;
                            metric.exercise = exercise_title.to_string();
                        }
                    }
                }
                if started.is_some_and(|value| value >= cutoff) {
                    let tonnage = effective_weight * reps;
                    *load.entry(primary.to_string()).or_default() += tonnage;
                    for muscle in &secondary {
                        *load.entry((*muscle).to_string()).or_default() += tonnage * 0.5;
                    }
                }
            }
        }
    }

    let max_load = load.values().copied().fold(0.0_f64, f64::max);
    let development = development
        .into_iter()
        .map(|(muscle, metric)| {
            json!({
                "muscle": muscle,
                "score": metric.score,
                "level": (metric.score * 5.0).ceil().clamp(0.0, 5.0) as u8,
                "bestE1rmKg": metric.best_e1rm_kg,
                "exercise": metric.exercise,
            })
        })
        .collect::<Vec<_>>();
    let load = load
        .into_iter()
        .map(|(muscle, tonnage)| {
            json!({
                "muscle": muscle,
                "tonnageKg": tonnage,
                "intensity": if max_load > 0.0 { tonnage / max_load } else { 0.0 },
            })
        })
        .collect::<Vec<_>>();

    json!({
        "bodyWeightKg": body_weight_kg,
        "needsBodyWeight": body_weight_kg.is_none(),
        "workoutCount": workout_count,
        "rangeDays": range_days,
        "development": development,
        "load": load,
    })
}

pub(crate) async fn body_snapshot(
    ark: &ArkHost,
    data_dir: &Path,
    range: &str,
) -> Result<Value, String> {
    let range_days = match range {
        "week" => 7,
        "month" => 30,
        "year" => 365,
        _ => return Err("РќРµРёР·РІРµСЃС‚РЅС‹Р№ РїРµСЂРёРѕРґ РЅР°РіСЂСѓР·РєРё".to_string()),
    };
    let objects = ark_request(
        ark,
        "list_objects_by_type",
        json!({ "type_id": WORKOUT_TYPE_ID }),
    )
    .await?
    .as_array()
    .cloned()
    .unwrap_or_default();
    Ok(body_metrics(
        &objects,
        read_config(data_dir).body_weight_kg,
        range_days,
    ))
}
