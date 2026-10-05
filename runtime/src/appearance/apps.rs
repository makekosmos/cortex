// Per-app appearance overrides. Engine-owned, stored under the same Engine
// data dir as the global settings file — never in an app's own config. The
// global `follow_apps` flag stays global and Manager-only: when it is true,
// scoped reads return the shared global settings and scoped writes are
// rejected, but any previously stored per-app file is left untouched and
// comes back into effect once `follow_apps` is turned off.

const APPS_DIR: &str = "appearance-apps";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppAppearanceSettings {
    pub schema_version: u8,
    pub mode: String,
    pub light_theme: String,
    pub dark_theme: String,
    pub accent_source: String,
    pub accent_color: Option<String>,
    pub material: String,
    pub font_family: String,
    pub font_size: f32,
    pub revision: u64,
}

impl From<&AppearanceSettings> for AppAppearanceSettings {
    fn from(global: &AppearanceSettings) -> Self {
        Self {
            schema_version: global.schema_version,
            mode: global.mode.clone(),
            light_theme: global.light_theme.clone(),
            dark_theme: global.dark_theme.clone(),
            accent_source: global.accent_source.clone(),
            accent_color: global.accent_color.clone(),
            material: global.material.clone(),
            font_family: global.font_family.clone(),
            font_size: global.font_size,
            revision: 0,
        }
    }
}

fn validate_app(settings: &AppAppearanceSettings) -> Result<(), String> {
    if settings.schema_version != 1 {
        return Err("schema_version must be 1".into());
    }
    validate_fields(
        &settings.mode,
        &settings.light_theme,
        &settings.dark_theme,
        &settings.accent_source,
        settings.accent_color.as_deref(),
        &settings.material,
        &settings.font_family,
        settings.font_size,
    )
}

/// `app_id` becomes a single path component (`<app_id>.json`): reject
/// anything that could traverse out of the per-app directory or that isn't a
/// plain, bounded identifier.
pub fn is_safe_app_id(app_id: &str) -> bool {
    if app_id.is_empty() || app_id.len() > 128 || app_id == "." || app_id == ".." {
        return false;
    }
    app_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn read_app(path: &Path, fallback: AppAppearanceSettings) -> Result<AppAppearanceSettings, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(fallback),
        Err(error) => return Err(error.to_string()),
    };
    let settings: AppAppearanceSettings = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid appearance settings file: {error}"))?;
    validate_app(&settings)?;
    Ok(settings)
}

#[derive(Debug, Clone, Serialize)]
struct AppearancePolicy {
    following: bool,
    editable: bool,
    can_set_follow_apps: bool,
}

impl AppearanceStore {
    fn app_path(&self, app_id: &str) -> PathBuf {
        self.path
            .parent()
            .expect("appearance path has a parent")
            .join(APPS_DIR)
            .join(format!("{app_id}.json"))
    }

    async fn get_scoped(&self, app_id: &str, client: &DispatchClient) -> Result<Value, String> {
        if !is_safe_app_id(app_id) {
            return Err("invalid app_id".into());
        }
        let global = read(&self.path)?;
        let can_set_follow_apps = client.class.as_deref() == Some("manager-gpui");
        let (settings, policy) = if global.follow_apps {
            (
                AppAppearanceSettings::from(&global),
                AppearancePolicy {
                    following: true,
                    editable: false,
                    can_set_follow_apps: false,
                },
            )
        } else {
            let fallback = AppAppearanceSettings::from(&global);
            (
                read_app(&self.app_path(app_id), fallback)?,
                AppearancePolicy {
                    following: false,
                    editable: true,
                    can_set_follow_apps,
                },
            )
        };
        Ok(self.app_response(app_id, settings, policy).await)
    }

    async fn set_scoped(
        &self,
        app_id: &str,
        patch: &serde_json::Map<String, Value>,
        client: &DispatchClient,
    ) -> Result<Value, String> {
        if !is_safe_app_id(app_id) {
            return Err("invalid app_id".into());
        }
        let authorized = client.class.as_deref() == Some(app_id)
            || client.class.as_deref() == Some("manager-gpui");
        if !authorized {
            return Err("client is not authorized to set appearance for this app_id".into());
        }
        if patch.is_empty() {
            return Err("appearance.set patch must not be empty".into());
        }
        if patch.contains_key("revision")
            || patch.contains_key("schema_version")
            || patch.contains_key("follow_apps")
        {
            return Err(
                "revision, schema_version and follow_apps cannot be set on a scoped patch".into(),
            );
        }

        let _guard = self.write_lock.lock().await;
        let global = read(&self.path)?;
        if global.follow_apps {
            return Err("appearance is following apps; per-app overrides are disabled".into());
        }
        let app_path = self.app_path(app_id);
        let fallback = AppAppearanceSettings::from(&global);
        let mut next = serde_json::to_value(read_app(&app_path, fallback)?)
            .map_err(|error| error.to_string())?;
        let object = next.as_object_mut().expect("serialized settings object");
        for (key, value) in patch {
            if !object.contains_key(key) {
                return Err(format!("unknown appearance field: {key}"));
            }
            object.insert(key.clone(), value.clone());
        }
        let mut settings: AppAppearanceSettings = serde_json::from_value(next)
            .map_err(|error| format!("invalid appearance patch: {error}"))?;
        validate_app(&settings)?;
        settings.revision = settings
            .revision
            .checked_add(1)
            .ok_or("appearance revision exhausted")?;
        persist(&app_path, &settings)?;
        drop(_guard);

        let policy = AppearancePolicy {
            following: false,
            editable: true,
            can_set_follow_apps: client.class.as_deref() == Some("manager-gpui"),
        };
        Ok(self.app_response(app_id, settings, policy).await)
    }

    async fn app_response(
        &self,
        app_id: &str,
        settings: AppAppearanceSettings,
        policy: AppearancePolicy,
    ) -> Value {
        let wants_wallpaper = settings.accent_source == "wallpaper";
        let mut value = json!({
            "app_id": app_id,
            "settings": settings,
            "policy": policy,
            "capabilities": {
                "materials": materials(),
                "wallpaper_accent": cfg!(target_os = "macos"),
            },
            "wallpaper_accent": null,
        });
        if wants_wallpaper {
            match self.wallpaper.snapshot().await {
                Ok(color) => value["wallpaper_accent"] = json!(color),
                Err(error) => value["wallpaper_error"] = json!(error),
            }
        }
        value
    }
}
