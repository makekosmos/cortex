//! The contract: what a plugin tells the Engine about itself.
//!
//! One file, `plugin.json`, is the whole of it — the Engine reads it from the
//! catalog to install, keeps it beside the install, and answers the pages from
//! it. Nothing in the Engine names a plugin: a new one is a new manifest.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::PluginError;

/// The format this Engine writes and understands. A manifest from a newer
/// format is refused rather than half-read.
pub const MANIFEST_VERSION: u32 = 1;

fn default_tag_prefix() -> String {
    "v".to_owned()
}

/// One plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// Format version — [`MANIFEST_VERSION`] here.
    pub schema_version: u32,
    /// Reverse-DNS id: the install directory, the settings file name, the
    /// identity the Manager remembers. Never reuse one for another plugin.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    pub source: PluginSource,
    /// What the person can change, in the order the page shows it.
    #[serde(default)]
    pub settings: Vec<SettingField>,
    /// Where the settings are kept, as a plain file name in the Engine's data
    /// directory. A plugin that had a config file before it was a plugin names
    /// it here (`dictation-config.json`); everything else gets
    /// `<plugin dir>/config.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_file: Option<String>,
}

/// Where a plugin's code comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginSource {
    /// A program published as GitHub release assets of `repository`:
    /// `<asset>-<version>-<target>.zip`, with `SHA256SUMS.txt` beside them.
    Release {
        repository: String,
        /// The prefix in front of the bare version in the tag: `v`, `gpui-v`.
        #[serde(default = "default_tag_prefix")]
        tag_prefix: String,
        /// Asset name stem, and the executable inside the archive.
        asset: String,
        /// The `.app` bundle name a darwin tarball ships; `asset` by default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bundle: Option<String>,
    },
    /// Part of the Manager itself — a page, an animation. Nothing to download.
    Builtin,
}

/// One setting the page can show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingField {
    /// The key in the settings file. This is the plugin's own spelling, so it
    /// matches what the plugin reads.
    pub key: String,
    pub label: String,
    pub kind: SettingKind,
    /// The value before the person changes anything.
    #[serde(default)]
    pub value: Value,
    /// A line under the field: what it does, or what it costs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettingKind {
    /// A line of text.
    Text,
    /// On or off.
    Toggle,
    /// A number between `min` and `max`.
    Number {
        #[serde(default)]
        min: f64,
        #[serde(default)]
        max: f64,
    },
    /// One of `options`, stored as its `value`.
    Choice { options: Vec<Choice> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

impl PluginManifest {
    /// Refuse a manifest the Engine cannot act on: a newer format, an id that
    /// could name a path of its own, duplicate keys, a source with no name.
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.schema_version != MANIFEST_VERSION {
            return Err(PluginError::Manifest(format!(
                "формат описания {} — движок понимает {MANIFEST_VERSION}",
                self.schema_version
            )));
        }
        if !valid_id(&self.id) {
            return Err(PluginError::Manifest(format!("id «{}»", self.id)));
        }
        if self.name.trim().is_empty() {
            return Err(PluginError::Manifest("у плагина нет имени".into()));
        }
        if let Some(file) = self.settings_file.as_deref() {
            if !valid_file_name(file) {
                return Err(PluginError::Manifest(format!("settings_file «{file}»")));
            }
        }
        let mut keys = std::collections::HashSet::new();
        for field in &self.settings {
            if !valid_key(&field.key) {
                return Err(PluginError::Manifest(format!("ключ «{}»", field.key)));
            }
            if !keys.insert(field.key.as_str()) {
                return Err(PluginError::Manifest(format!(
                    "ключ «{}» объявлен дважды",
                    field.key
                )));
            }
        }
        match &self.source {
            PluginSource::Release {
                repository, asset, ..
            } => {
                if !valid_repository(repository) {
                    return Err(PluginError::Manifest(format!("репозиторий «{repository}»")));
                }
                if asset.trim().is_empty() || asset.contains(['/', '\\', ' ']) {
                    return Err(PluginError::Manifest(format!("файл «{asset}»")));
                }
            }
            PluginSource::Builtin => {}
        }
        Ok(())
    }

    /// The settings of `values` that the manifest declares, each coerced to its
    /// field's kind. Anything else is refused: a page that sends a key the
    /// plugin does not read is a page with a bug, and silently storing it would
    /// hide that.
    pub fn check_settings(
        &self,
        values: &Map<String, Value>,
    ) -> Result<Map<String, Value>, PluginError> {
        let mut checked = Map::new();
        for (key, value) in values {
            let field = self
                .settings
                .iter()
                .find(|field| &field.key == key)
                .ok_or_else(|| {
                    PluginError::Settings(format!("плагин не знает настройки «{key}»"))
                })?;
            checked.insert(key.clone(), field.coerce(value)?);
        }
        Ok(checked)
    }
}

impl SettingField {
    /// The value as this field stores it, or why it cannot.
    fn coerce(&self, value: &Value) -> Result<Value, PluginError> {
        match &self.kind {
            SettingKind::Text => match value {
                Value::String(text) if text.len() <= 4096 => Ok(value.clone()),
                _ => Err(PluginError::Settings(format!("«{}» — не текст", self.key))),
            },
            SettingKind::Toggle => match value {
                Value::Bool(_) => Ok(value.clone()),
                _ => Err(PluginError::Settings(format!(
                    "«{}» — не включено/выключено",
                    self.key
                ))),
            },
            SettingKind::Number { min, max } => match value.as_f64() {
                Some(number) if number >= *min && number <= *max => Ok(value.clone()),
                _ => Err(PluginError::Settings(format!(
                    "«{}» — не число от {min} до {max}",
                    self.key
                ))),
            },
            SettingKind::Choice { options } => match value {
                Value::String(text) if options.iter().any(|option| &option.value == text) => {
                    Ok(value.clone())
                }
                _ => Err(PluginError::Settings(format!(
                    "«{}» — не один из вариантов",
                    self.key
                ))),
            },
        }
    }
}

/// A plugin id: reverse-DNS, lowercase, and never a path. Same shape as an app
/// id, because it is the same directory name.
pub fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('.')
        && value.contains('.')
        && value.bytes().all(id_byte)
}

/// What an id may be made of: a lowercase, dot-separated name.
fn id_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
}

/// A settings file name: one plain segment of the Engine's data directory.
pub fn valid_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.contains(['/', '\\', ':', '\0'])
        && !value.starts_with('.')
        && !value.ends_with('.')
        && !value.ends_with(' ')
}

/// A settings key: what the plugin's own config calls the field.
fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

/// `org/name`, the only shape a release repository can have.
fn valid_repository(value: &str) -> bool {
    let mut parts = value.split('/');
    let (Some(org), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !org.is_empty()
        && !name.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
}

#[cfg(test)]
mod tests {
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
        };
        assert_eq!(field.coerce(&json!(16_000)).unwrap(), json!(16_000));
        assert!(field.coerce(&json!(4_000)).is_err());
        assert!(field.coerce(&json!("16000")).is_err());
    }
}
