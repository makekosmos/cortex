fn make_tracked_app(id: &str) -> TrackedApp {
    TrackedApp {
        id: id.to_string(),
        platform: "windows".to_string(),
        exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
        normalized_exe_path: r"c:\\apps\\demo\\demo.exe".to_string(),
        process_name: "demo.exe".to_string(),
        display_name: Some("Demo App".to_string()),
        publisher: Some("Demo Corp".to_string()),
        icon_ref: None,
        first_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
        last_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
    }
}

fn make_usage_session(id: &str, tracked_app_id: &str) -> UsageSession {
    UsageSession {
        id: id.to_string(),
        tracked_app_id: tracked_app_id.to_string(),
        device_id: "device-1".to_string(),
        device_name: "Test Device".to_string(),
        platform: "windows".to_string(),
        started_at: "2026-01-01T00:00:00.000Z".to_string(),
        ended_at: Some("2026-01-01T00:10:00.000Z".to_string()),
        runtime_ms: 600_000,
        foreground_ms: 600_000,
        idle_ms: 0,
        window_title: Some("Demo Window".to_string()),
        process_name: "demo.exe".to_string(),
        exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
        pid_start: Some(1234),
        pid_end: Some(1234),
        meta_json: json!({"note": "session"}),
    }
}

fn make_usage_event(id: &str, tracked_app_id: &str, session_id: Option<&str>) -> UsageEvent {
    UsageEvent {
        id: id.to_string(),
        tracked_app_id: tracked_app_id.to_string(),
        usage_session_id: session_id.map(|value| value.to_string()),
        device_id: "device-1".to_string(),
        device_name: "Test Device".to_string(),
        platform: "windows".to_string(),
        occurred_at: "2026-01-01T00:05:00.000Z".to_string(),
        kind: "foreground".to_string(),
        window_title: Some("Demo Window".to_string()),
        process_name: "demo.exe".to_string(),
        exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
        pid: Some(1234),
        is_foreground: true,
        is_idle: false,
        meta_json: json!({"note": "event"}),
    }
}

fn make_object(id: &str, type_id: &str, title: &str) -> ArkObject {
    let mut object = ArkObject {
        id: id.to_string(),
        type_id: type_id.to_string(),
        type_version: "0.0.0-legacy".to_string(),
        title: title.to_string(),
        content_json: json!({
            "type": "doc",
            "content": [{ "type": "paragraph" }]
        }),
        props_json: json!({
            "description": format!("Description for {title}")
        }),
        created_at: "2026-01-01T00:00:00.000Z".to_string(),
        updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        deleted_at: None,
    };
    if type_id == "core.note" {
        object.type_version = "0.1.0".into();
        object.props_json["extensions"] = json!({});
    }
    object
}
