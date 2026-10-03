//! Engine-backed appearance snapshot; failures preserve the last applied style.
use gpui::{App, FocusHandle, Window, WindowAppearance};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

#[cfg(test)]
#[path = "appearance_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub mode: String,
    pub light_theme: String,
    pub dark_theme: String,
    pub accent_source: String,
    pub accent_color: Option<String>,
    pub follow_apps: bool,
    pub material: String,
    pub font_family: String,
    pub font_size: f32,
    pub revision: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            mode: "dark".into(),
            light_theme: "default".into(),
            dark_theme: "default".into(),
            accent_source: "theme".into(),
            accent_color: None,
            follow_apps: false,
            material: "opaque".into(),
            font_family: "Inter".into(),
            font_size: 13.,
            revision: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub dark: bool,
    pub theme_index: usize,
    pub accent: Option<u32>,
    pub material: String,
    pub font_family: String,
    pub font_size: f32,
    pub mode: String,
}

/// Transient selector state; the Engine snapshot remains the sole source of values.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AppearanceMenu {
    LightTheme,
    DarkTheme,
    FontFamily,
    FontSize,
}

pub struct Appearance {
    pub settings: Settings,
    pub ready: bool,
    pub materials: Vec<String>,
    pub wallpaper_supported: bool,
    pub wallpaper_accent: Option<u32>,
    pub wallpaper_error: Option<String>,
    pub fonts: Vec<String>,
    pub font_menu_open: bool,
    pub open_menu: Option<AppearanceMenu>,
    pub menu_highlighted: usize,
    pub menu_dismissed_at: Option<(AppearanceMenu, Instant)>,
    pub menu_query: String,
    pub menu_generation: u64,
    pub tile_focus: HashMap<String, FocusHandle>,
    pub applied: Option<Resolved>,
    pub next_poll: Instant,
}

impl Appearance {
    pub fn new(cx: &App) -> Self {
        let mut fonts = cx.text_system().all_font_names();
        fonts.push(".SystemUIFont".into());
        fonts.sort_by_key(|name| name.to_lowercase());
        fonts.dedup();
        Self {
            settings: Settings::default(),
            ready: false,
            materials: vec!["default".into(), "opaque".into()],
            wallpaper_supported: false,
            wallpaper_accent: None,
            wallpaper_error: None,
            fonts,
            font_menu_open: false,
            open_menu: None,
            menu_highlighted: 0,
            menu_dismissed_at: None,
            menu_query: String::new(),
            menu_generation: 0,
            tile_focus: HashMap::new(),
            applied: None,
            next_poll: Instant::now(),
        }
    }

    pub fn ingest(&mut self, response: &Value) -> bool {
        let Ok(settings) = serde_json::from_value::<Settings>(response["settings"].clone()) else {
            return false;
        };
        if settings.schema_version != 1
            || !settings.font_size.is_finite()
            || !(11. ..=18.).contains(&settings.font_size)
        {
            return false;
        }
        self.settings = settings;
        self.ready = true;
        self.materials = response["capabilities"]["materials"]
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_else(|| vec!["default".into(), "opaque".into()]);
        self.wallpaper_supported = response["capabilities"]["wallpaper_accent"]
            .as_bool()
            .unwrap_or(false);
        self.wallpaper_accent = response["wallpaper_accent"].as_str().and_then(parse_color);
        self.wallpaper_error = response["wallpaper_error"].as_str().map(str::to_owned);
        true
    }

    pub fn resolve(&self, window: &Window) -> Resolved {
        self.resolve_dark(matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ))
    }

    pub fn resolve_dark(&self, system_dark: bool) -> Resolved {
        let s = &self.settings;
        let dark = match s.mode.as_str() {
            "light" => false,
            "dark" => true,
            _ => system_dark,
        };
        let key = if dark { &s.dark_theme } else { &s.light_theme };
        let theme_index = imago_gpui::palettes::THEMES
            .iter()
            .position(|theme| theme.key == key)
            .unwrap_or(0);
        let accent = match s.accent_source.as_str() {
            "custom" => s.accent_color.as_deref().and_then(parse_color),
            "wallpaper" => self.wallpaper_accent,
            _ => None,
        };
        let material = if self.materials.contains(&s.material) {
            s.material.clone()
        } else {
            "opaque".into()
        };
        let family = if self.fonts.contains(&s.font_family) {
            s.font_family.clone()
        } else {
            ".SystemUIFont".into()
        };
        Resolved {
            dark,
            theme_index,
            accent,
            material,
            font_family: family,
            font_size: s.font_size,
            mode: s.mode.clone(),
        }
    }
}

pub fn parse_color(value: &str) -> Option<u32> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}
