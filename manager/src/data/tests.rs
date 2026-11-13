//! The mappings above, exercised on the answers a live Engine gave.

use serde_json::json;

use super::rows::size;
use super::{apply, shape, write};

#[test]
fn sizes_read_as_the_page_wants() {
    assert_eq!(size(940), "940 Б");
    assert_eq!(size(1_800_000_000), "1.8 ГБ");
    assert_eq!(size(24_500_000_000), "25 ГБ");
}

/// The Devices page reads its rows from `sync` and nowhere else: one catalog,
/// so an Engine without peers leaves an empty list rather than a second copy of
/// the devices that disagrees with the first.
#[test]
fn a_device_list_survives_an_engine_without_peers() {
    let sync = shape("sync", json!({ "running": false, "peers": [] }));
    assert_eq!(sync["peers"], json!([]));
    assert_eq!(sync["incoming"], json!([]));
}

#[test]
fn the_settings_call_reads_only_what_it_owns() {
    // `engine.settings.get` answers the usage-tracker switch; it says nothing
    // about autostart, so it must not write one.
    let settings = shape(
        "settings",
        json!({ "enabled": true, "usage_tracker": { "enabled": true } }),
    );
    assert_eq!(settings["telemetry"], true);
    assert!(settings.get("autostart").is_none());
}

/// The route the `settings` page takes: the call lands under `settings.autostart`
/// and the switch reads it there. The Engine answers an object, the state holds
/// the position — an object would be truthy in JavaScript and pin the switch on.
#[test]
fn autostart_reaches_the_switch_as_a_boolean() {
    let mut state = json!({ "settings": { "autostart": false, "telemetry": false } });

    apply(
        &mut state,
        "settings.autostart",
        json!({ "available": true, "enabled": true, "reason": "registry" }),
        None,
    );
    assert_eq!(state["settings"]["autostart"], json!(true));

    apply(
        &mut state,
        "settings.autostart",
        json!({ "available": false, "enabled": false, "reason": "unsupported" }),
        None,
    );
    assert_eq!(state["settings"]["autostart"], json!(false));

    apply(
        &mut state,
        "settings.autostart",
        json!({ "available": true, "reason": "no value yet" }),
        None,
    );
    assert_eq!(state["settings"]["autostart"], json!(false));
}

/// Both calls land on `settings`, in either order: the page asks for the
/// settings object and for the autostart switch together, and neither answer
/// may wipe the other's field.
#[test]
fn the_two_settings_answers_do_not_wipe_each_other() {
    let mut state = json!({ "settings": { "autostart": true, "telemetry": false } });

    apply(
        &mut state,
        "settings",
        json!({ "enabled": true, "usage_tracker": { "enabled": true } }),
        None,
    );
    assert_eq!(state["settings"]["autostart"], json!(true));
    assert_eq!(state["settings"]["telemetry"], json!(true));

    // And a failed autostart call writes nothing: the switch keeps its last
    // known position instead of falling to a `false` from another answer.
    apply(
        &mut state,
        "settings.autostart",
        json!({ "available": true, "enabled": false, "reason": "registry" }),
        None,
    );
    assert_eq!(state["settings"]["autostart"], json!(false));
    assert_eq!(state["settings"]["telemetry"], json!(true));
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
    let models = json!({
        "models": [
            { "id": "whisper-small", "name": "Whisper Small", "sizeMb": 466, "downloaded": true },
        ],
    });
    let rows = shape("models", models);
    assert_eq!(rows[0]["size"], "466 МБ");
    assert_eq!(rows[0]["downloaded"], true);
}

#[test]
fn a_pairing_request_carries_who_is_asking() {
    let snapshot = json!({
        "running": true,
        "incoming_pairing_requests": [{
            "device_id": "d1",
            "device_name": "Мастерская",
            "platform": "windows",
            "app_version": "0.10.26",
        }],
        "peers": [{ "device_id": "d2", "device_name": "Pixel", "status": "online" }],
    });
    let sync = shape("sync", snapshot);
    assert_eq!(sync["incoming"][0]["name"], "Мастерская");
    assert_eq!(sync["incoming"][0]["id"], "d1");
    assert_eq!(sync["peers"][0]["status"], "online");
}

#[test]
fn updates_say_what_the_channel_offers() {
    let status = json!({
        "state": "available",
        "currentVersion": "0.10.26",
        "newVersion": "0.10.27",
        "canInstall": true,
        "percent": 0,
    });
    let updates = shape("updates", status);
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
