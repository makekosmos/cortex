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
pub fn ACCENT_FG() -> u32 {
    ACCENT_OVERRIDE
        .with(Cell::get)
        .map(accent_foreground)
        .unwrap_or_else(imago_gpui::theme::ACCENT_FG)
}
pub fn window_surface(color: u32) -> Hsla {
    rgba(
        color,
        if TRANSLUCENT.with(Cell::get) {
            0.82
        } else {
            1.
        },
    )
}

pub fn apply(profile: &Resolved, window: &Window, cx: &mut App) {
    imago_gpui::theme::set_mode(profile.dark);
    imago_gpui::theme::set_theme(profile.theme_index);
    imago_gpui::theme::apply(cx);
    ACCENT_OVERRIDE.with(|slot| slot.set(profile.accent));
    FONT_SCALE.with(|slot| slot.set(profile.font_size / 13.));
    let translucent = matches!(profile.material.as_str(), "frosted" | "acrylic" | "mica");
    TRANSLUCENT.with(|slot| slot.set(translucent));
    let background = match profile.material.as_str() {
        "frosted" | "acrylic" => WindowBackgroundAppearance::Blurred,
        "mica" => WindowBackgroundAppearance::MicaBackdrop,
        _ => WindowBackgroundAppearance::Opaque,
    };
    window.set_background_appearance(background);
    cx.set_window_appearance(match profile.mode.as_str() {
        "light" => Some(WindowAppearance::Light),
        "dark" => Some(WindowAppearance::Dark),
        _ => None,
    });
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
}
