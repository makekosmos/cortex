//! The mappings above, exercised on the answers a live Engine gave.

use super::*;

use super::*;

#[test]
fn sizes_read_as_the_page_wants() {
    assert_eq!(size(940), "940 Б");
    assert_eq!(size(1_800_000_000), "1.8 ГБ");
    assert_eq!(size(24_500_000_000), "25 ГБ");
}

#[test]
fn a_device_list_survives_an_engine_without_peers() {
    let rows = shape("devices", json!({ "running": false, "peers": [] }));
    assert_eq!(rows, json!([]));
}

#[test]
fn an_empty_answer_keeps_the_page_readable() {
    let settings = shape("settings", json!({ "available": true, "enabled": true }));
    assert_eq!(settings["autostart"], true);
}

#[test]
fn usage_counts_minutes_from_the_engines_milliseconds() {
    let usage = shape(
        "usage",
        json!({
            "dailyTrend": [
                { "date": "2026-11-01", "foregroundMs": 600_000 },
                { "date": "2026-11-02", "foregroundMs": 1_800_000 },
            ],
            "topApps": [{ "displayName": "Figma", "foregroundMs": 1_200_000 }],
        }),
    );
    assert_eq!(usage["todayMinutes"], 30);
    assert_eq!(usage["weekMinutes"], 40);
    assert_eq!(usage["top"][0]["minutes"], 20);
}

#[test]
fn a_model_states_its_size_in_megabytes() {
    let rows = shape(
        "models",
        json!({ "models": [{ "id": "whisper-small", "name": "Whisper Small", "sizeMb": 466, "downloaded": true }] }),
    );
    assert_eq!(rows[0]["size"], "466 МБ");
    assert_eq!(rows[0]["downloaded"], true);
}

#[test]
fn a_pairing_request_carries_who_is_asking() {
    let sync = shape(
        "sync",
        json!({
            "running": true,
            "incoming_pairing_requests": [
                { "device_id": "d1", "device_name": "Мастерская", "platform": "windows", "app_version": "0.10.26" }
            ],
            "peers": [{ "device_id": "d2", "device_name": "Pixel", "status": "online" }],
        }),
    );
    assert_eq!(sync["incoming"][0]["name"], "Мастерская");
    assert_eq!(sync["incoming"][0]["id"], "d1");
    assert_eq!(sync["peers"][0]["status"], "online");
}

#[test]
fn appearance_reads_the_settings_the_engine_stores() {
    let theme = shape(
        "appearance",
        json!({ "settings": { "mode": "dark", "light_theme": "merino-light", "dark_theme": "cobalt-foundry", "follow_apps": true } }),
    );
    assert_eq!(theme["dark"], "cobalt-foundry");
    assert_eq!(theme["followApps"], true);
}

#[test]
fn updates_say_what_the_channel_offers() {
    let updates = shape(
        "updates",
        json!({ "state": "available", "currentVersion": "0.10.26", "newVersion": "0.10.27", "canInstall": true, "percent": 0 }),
    );
    assert_eq!(updates["newVersion"], "0.10.27");
    assert_eq!(updates["canInstall"], true);
}

#[test]
fn keys_never_leak_a_secret() {
    let keys = shape(
        "keys",
        json!({ "hasApiKey": true, "config": { "language": "ru" } }),
    );
    assert_eq!(keys["hasApiKey"], true);
    assert!(keys.get("key").is_none());
}

#[test]
fn writing_a_path_creates_the_objects_above_it() {
    let mut state = json!({ "engine": { "status": "running" } });
    write(&mut state, "settings.autostart", json!(true));
    assert_eq!(state["settings"]["autostart"], true);
    assert_eq!(state["engine"]["status"], "running");
}
