#![allow(clippy::unwrap_used)]

use super::*;
use serde_json::json;

fn manifest() -> PluginManifest {
    serde_json::from_value(json!({
        "schema_version": 1,
        "id": "com.kosmos.dictation",
        "name": "Диктовка",
        "summary": "Речь в текст по горячей клавише",
        "source": {
            "kind": "release",
            "repository": "makekosmos/dictation",
            "tag_prefix": "gpui-v",
            "asset": "dictation-gpui"
        },
        "settings_file": "dictation-config.json",
        "settings": [
            {
                "key": "hotkey", "label": "Клавиша",
                "kind": { "type": "text" }, "value": "Ctrl+Shift+;"
            },
            { "key": "language", "label": "Язык", "kind": { "type": "choice", "options": [
                { "value": "auto", "label": "Автоматически" },
                { "value": "ru", "label": "Русский" }
            ] }, "value": "auto" }
        ]
    }))
    .expect("manifest")
}

#[test]
fn a_manifest_round_trips_through_json() {
    let parsed = manifest();
    parsed.validate().expect("valid");
    let text = serde_json::to_string(&parsed).unwrap();
    let again: PluginManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(again, parsed);
    assert_eq!(again.source.target().unwrap().tag_prefix, "gpui-v");
}

#[test]
fn a_newer_format_is_refused() {
    let mut parsed = manifest();
    parsed.schema_version = MANIFEST_VERSION + 1;
    assert!(parsed.validate().is_err());
}

#[test]
fn an_id_that_could_name_a_path_is_refused() {
    for id in ["", "..", "a/b", "Com.Mundus", "nodot", ".hidden", "a b"] {
        let mut parsed = manifest();
        parsed.id = id.to_owned();
        assert!(parsed.validate().is_err(), "id {id:?} should be refused");
    }
}

#[test]
fn a_settings_file_with_a_path_is_refused() {
    for file in ["../x.json", "sub/x.json", "", ".hidden", "C:x.json"] {
        let mut parsed = manifest();
        parsed.settings_file = Some(file.to_owned());
        assert!(
            parsed.validate().is_err(),
            "file {file:?} should be refused"
        );
    }
    let mut parsed = manifest();
    parsed.settings_file = Some("dictation-config.json".to_owned());
    parsed.validate().expect("a plain file name is fine");
}

#[test]
fn duplicate_setting_keys_are_refused() {
    let mut parsed = manifest();
    parsed.settings.push(parsed.settings[0].clone());
    assert!(parsed.validate().is_err());
}

#[test]
fn settings_are_coerced_to_their_field() {
    let parsed = manifest();
    let mut values = Map::new();
    values.insert("hotkey".into(), json!("Ctrl+Alt+D"));
    values.insert("language".into(), json!("ru"));
    let checked = parsed.check_settings(&values).unwrap();
    assert_eq!(checked["hotkey"], json!("Ctrl+Alt+D"));

    values.insert("language".into(), json!("klingon"));
    assert!(parsed.check_settings(&values).is_err());
    values.remove("language");
    values.insert("unknown".into(), json!("x"));
    assert!(parsed.check_settings(&values).is_err());
}

#[test]
fn a_switch_field_stores_one_of_its_two_values() {
    let field = SettingField {
        key: "injectMode".into(),
        label: "Вставлять текст автоматически".into(),
        kind: SettingKind::Switch {
            on: json!("auto_paste"),
            off: json!("clipboard_only"),
        },
        value: json!("auto_paste"),
        hint: None,
        apply: None,
    };
    assert_eq!(
        field.coerce(&json!("clipboard_only")).unwrap(),
        json!("clipboard_only")
    );
    assert!(field.coerce(&json!(true)).is_err());
}

#[test]
fn settings_are_applied_through_the_ops_they_name() {
    let mut parsed = manifest();
    parsed.settings_op = Some("dictation.update_config".into());
    parsed.settings[1].apply = Some(FieldApply {
        op: "dictation.use_local_model".into(),
        param: Some("modelId".into()),
    });
    parsed.validate().expect("valid");

    // Two fields, two operations: the patch and the model picker.
    let mut values = Map::new();
    values.insert("hotkey".into(), json!("Ctrl+Alt+D"));
    values.insert("language".into(), json!("ru"));
    let checked = parsed.check_settings(&values).unwrap();
    let applies = parsed.applies(&checked);
    assert_eq!(applies.len(), 2);
    assert_eq!(applies[0].op, "dictation.update_config");
    assert_eq!(applies[0].params["hotkey"], json!("Ctrl+Alt+D"));
    assert_eq!(applies[1].op, "dictation.use_local_model");
    assert_eq!(applies[1].params["modelId"], json!("ru"));

    // A field with no operation of its own rides the manifest's.
    let mut values = Map::new();
    values.insert("language".into(), json!("ru"));
    let mut parsed = manifest();
    parsed.settings_op = Some("dictation.update_config".into());
    let checked = parsed.check_settings(&values).unwrap();
    let applies = parsed.applies(&checked);
    assert_eq!(applies.len(), 1);
    assert_eq!(applies[0].params["language"], json!("ru"));

    // No operation named: nothing to apply, the file is all there is.
    let mut parsed = manifest();
    parsed.settings_op = None;
    let checked = parsed.check_settings(&values).unwrap();
    assert!(parsed.applies(&checked).is_empty());
}

#[test]
fn a_number_field_keeps_its_range() {
    let field = SettingField {
        key: "sample_rate".into(),
        label: "Частота".into(),
        kind: SettingKind::Number {
            min: 8_000.0,
            max: 48_000.0,
        },
        value: json!(16_000),
        hint: None,
        apply: None,
    };
    assert_eq!(field.coerce(&json!(16_000)).unwrap(), json!(16_000));
    assert!(field.coerce(&json!(4_000)).is_err());
    assert!(field.coerce(&json!("16000")).is_err());
}

#[test]
fn a_plugin_says_how_it_starts_and_that_survives_a_round_trip() {
    let mut parsed = manifest();
    parsed.start = Some(PluginStart {
        args: vec!["--background".into()],
    });
    parsed.validate().expect("valid");
    let text = serde_json::to_string(&parsed).unwrap();
    let again: PluginManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(again.start, parsed.start);
    // A manifest that says nothing about starting stays silent in its JSON.
    let quiet = serde_json::to_string(&manifest()).unwrap();
    assert!(!quiet.contains("\"start\""));
}

#[test]
fn a_start_line_the_engine_will_not_run_is_refused() {
    let mut long = manifest();
    long.start = Some(PluginStart {
        args: (0..17).map(|index| format!("--a{index}")).collect(),
    });
    assert!(long.validate().is_err(), "seventeen arguments");

    let mut empty = manifest();
    empty.start = Some(PluginStart {
        args: vec![String::new()],
    });
    assert!(empty.validate().is_err(), "an empty argument");

    let mut nul = manifest();
    nul.start = Some(PluginStart {
        args: vec!["--background\0".into()],
    });
    assert!(nul.validate().is_err(), "an argument with a NUL");
}

#[test]
fn a_builtin_plugin_cannot_pretend_to_have_a_program() {
    let mut parsed = manifest();
    parsed.source = PluginSource::Builtin;
    parsed.start = Some(PluginStart::default());
    assert!(parsed.validate().is_err());
}
