//! App-local accent and typography overlay; never modifies the dependency cache.
#![allow(non_snake_case)]
use crate::appearance_state::Resolved;
use gpui::{px, App, Hsla, Pixels, Window, WindowAppearance, WindowBackgroundAppearance};
pub use imago_gpui::theme::{mix, rgba, SIDEBAR_BG};
pub use mundus_gpui_kit::theme::*;
use std::cell::Cell;

thread_local! {
    static ACCENT_OVERRIDE: Cell<Option<u32>> = const { Cell::new(None) };
    static FONT_SCALE: Cell<f32> = const { Cell::new(1.) };
    static TRANSLUCENT: Cell<bool> = const { Cell::new(false) };
    static WINDOW_CHROME: Cell<Option<(WindowBackgroundAppearance, Option<WindowAppearance>)>> =
        const { Cell::new(None) };
}

pub fn ui_px(base: f32) -> Pixels {
    px(base * FONT_SCALE.with(Cell::get))
}
pub fn ACCENT() -> u32 {
    ACCENT_OVERRIDE
        .with(Cell::get)
        .unwrap_or_else(imago_gpui::theme::ACCENT)
}
pub fn accent_foreground(accent: u32) -> u32 {
    let channel = |shift: u32| {
        let value = ((accent >> shift) & 255u32) as f32 / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0);
    if luminance > 0.179 {
        0x000000
    } else {
        0xffffff
    }
}
/// Zeron glass is a surface recipe, not a single window flag. macOS and
/// Windows blur the desktop; Linux stays solid because compositor blur is not
/// guaranteed.
pub fn is_glass() -> bool {
    TRANSLUCENT.with(Cell::get) && cfg!(any(target_os = "macos", target_os = "windows"))
}

/// Shell tint over the blurred desktop: the sidebar tone at Zeron's 0.80 glass
/// alpha. Opaque material keeps the solid page background.
pub fn shell_fill() -> Hsla {
    if is_glass() {
        rgba(SIDEBAR_BG(), 0.80)
    } else {
        c(BG())
    }
}

/// The sidebar sits directly on the frost shell. Opaque material paints its
/// own solid tone.
pub fn sidebar_fill() -> Hsla {
    if is_glass() {
        rgba(0, 0.)
    } else {
        c(SIDEBAR_BG())
    }
}

/// Main panel over the shell. Zeron uses the page color at 0.40 so content
/// stays readable without becoming a solid slab.
pub fn panel_fill() -> Hsla {
    if is_glass() {
        rgba(BG(), 0.40)
    } else {
        rgba(0, 0.)
    }
}

pub fn menu_fill() -> Hsla {
    if is_glass() {
        rgba(CARD(), 0.72)
    } else {
        c(CARD())
    }
}

pub fn apply(profile: &Resolved, window: &Window, cx: &mut App) {
    imago_gpui::theme::set_mode(profile.dark);
    imago_gpui::theme::set_theme(profile.theme_index);
    imago_gpui::theme::apply(cx);
    ACCENT_OVERRIDE.with(|slot| slot.set(profile.accent));
    FONT_SCALE.with(|slot| slot.set(profile.font_size / 13.));
    let glass = matches!(profile.material.as_str(), "frosted" | "acrylic" | "mica")
        && cfg!(any(target_os = "macos", target_os = "windows"));
    TRANSLUCENT.with(|slot| slot.set(glass));
    let background = if !glass {
        WindowBackgroundAppearance::Opaque
    } else if profile.material == "mica" {
        WindowBackgroundAppearance::MicaBackdrop
    } else {
        WindowBackgroundAppearance::Blurred
    };
    let appearance = match profile.mode.as_str() {
        "light" => Some(WindowAppearance::Light),
        "dark" => Some(WindowAppearance::Dark),
        _ => None,
    };
    // Repeating these native calls on every accent or font change resizes the
    // window chrome. Update them only when the requested chrome changes.
    if WINDOW_CHROME.with(|slot| {
        let next = (background, appearance);
        if slot.get() == Some(next) {
            false
        } else {
            slot.set(Some(next));
            true
        }
    }) {
        window.set_background_appearance(background);
        cx.set_window_appearance(appearance);
    }
    let theme = gpui_component::Theme::global_mut(cx);
    theme.font_family = profile.font_family.clone().into();
    // Rem geometry stays physical and aligned; explicit typography scales independently.
    theme.font_size = px(16.);
    if let Some(accent) = profile.accent {
        let color = c(accent);
        let foreground = c(accent_foreground(accent));
        let colors = &mut theme.colors;
        colors.accent = color;
        colors.accent_foreground = foreground;
        colors.primary = color;
        colors.primary_foreground = foreground;
        colors.primary_hover = mix(accent, 0.85, accent_foreground(accent));
        colors.primary_active = mix(accent, 0.72, accent_foreground(accent));
        colors.button_primary = color;
        colors.button_primary_foreground = foreground;
        colors.button_primary_hover = colors.primary_hover;
        colors.button_primary_active = colors.primary_active;
        colors.ring = color;
        colors.progress_bar = color;
        colors.slider_bar = color;
        colors.switch = color;
        colors.selection = rgba(accent, 0.3);
        colors.link = color;
        colors.tab_active = color;
        colors.sidebar_primary = color;
        colors.sidebar_primary_foreground = foreground;
    }
    // Switches, checks and tabs read `tokens`, not the legacy color fields.
    // Theme changes rebuild both; an accent-only change must do the same.
    theme.tokens = gpui_component::ThemeTokens::from(&theme.colors);
}
