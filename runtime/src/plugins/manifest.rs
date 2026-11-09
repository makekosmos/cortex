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
    /// The Engine operation the settings are applied through, for every field
    /// that does not name one of its own: `dictation.update_config` turns a
    /// patch into live state and writes the file the plugin reads. A manifest
    /// without it only has its file written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_op: Option<String>,
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
    /// The Engine operation this one field goes through, when the plugin's own
    /// op does not take it — a model picker is not a config patch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply: Option<FieldApply>,
}

/// One field's own Engine operation, and the parameter name it takes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldApply {
    pub op: String,
    /// The parameter the value is sent as; the field's key by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

/// One Engine call the Engine makes after storing settings: `op` with the
/// values of every field that names it, gathered into one `params`.
#[derive(Debug, Clone, PartialEq)]
pub struct Apply {
    pub op: String,
    pub params: Map<String, Value>,
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
    /// One of `options`, stored as its `value` — a string, a number, or null
    /// for a setting that means "never".
    Choice { options: Vec<Choice> },
    /// An on/off row whose two sides are values rather than `true`/`false`.
    Switch {
        #[serde(default = "on_value")]
        on: Value,
        #[serde(default)]
        off: Value,
    },
}

fn on_value() -> Value {
    Value::Bool(true)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: Value,
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
        if let Some(op) = self.settings_op.as_deref() {
            if !valid_op(op) {
                return Err(PluginError::Manifest(format!("операция настроек «{op}»")));
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
            if let Some(apply) = field.apply.as_ref() {
                if !valid_op(&apply.op) {
                    return Err(PluginError::Manifest(format!(
                        "операция настройки «{}»",
                        apply.op
                    )));
                }
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

    /// The Engine calls `values` turn into, in the manifest's own order: one
    /// call per operation, carrying every field that names it. Empty when the
    /// manifest names no operation — then the settings only live in the file.
    pub fn applies(&self, values: &Map<String, Value>) -> Vec<Apply> {
        let mut applies: Vec<Apply> = Vec::new();
        for field in &self.settings {
            let Some(value) = values.get(&field.key) else {
                continue;
            };
            let (op, param) = match field.apply.as_ref() {
                Some(apply) => (Some(apply.op.clone()), apply.param.as_deref()),
                None => (self.settings_op.clone(), None),
            };
            let Some(op) = op else { continue };
            let param = param.unwrap_or(&field.key);
            let params = match applies.iter_mut().find(|apply| apply.op == op) {
                Some(apply) => &mut apply.params,
                None => {
                    applies.push(Apply {
                        op: op.clone(),
                        params: Map::new(),
                    });
                    &mut applies.last_mut().expect("just pushed").params
                }
            };
            params.insert(param.to_owned(), value.clone());
        }
        applies
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
            SettingKind::Choice { options } => {
                if options.iter().any(|option| &option.value == value) {
                    Ok(value.clone())
                } else {
                    Err(PluginError::Settings(format!(
                        "«{}» — не один из вариантов",
                        self.key
                    )))
                }
            }
            SettingKind::Switch { on, off } => {
                if value == on || value == off {
                    Ok(value.clone())
                } else {
                    Err(PluginError::Settings(format!(
                        "«{}» — не одно из двух положений",
                        self.key
                    )))
                }
            }
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

/// An Engine operation name: `plugins.settings.set`, `dictation.update_config`.
/// A manifest may only name one, never build one.
fn valid_op(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.contains('.')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_')
        })
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
#[path = "manifest_tests.rs"]
mod tests;
