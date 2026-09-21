//! Manager palette — the "default" theme dark column from `imago-gpui`
//! (the same zeron values this file used to inline). The shell is
//! single-mode dark, so tokens are pinned as consts; palette updates flow
//! in with the imago-gpui rev pin.

use imago_gpui::palettes::THEMES;
use imago_gpui::theme::Palette;
pub use imago_gpui::theme::{c, rgba as fade};

const DARK: &Palette = &THEMES[0].dark;

pub const BG: u32 = DARK.bg;
pub const FG: u32 = DARK.fg;
pub const MUTED_FG: u32 = DARK.muted_fg;
pub const BORDER: u32 = DARK.border;
pub const ACCENT: u32 = DARK.accent;
#[allow(dead_code)]
pub const ACCENT_FG: u32 = DARK.accent_fg;
#[allow(dead_code)]
pub const ACCENT_DIM: u32 = DARK.accent_dim;
#[allow(dead_code)]
pub const SECONDARY: u32 = DARK.secondary;
pub const CARD: u32 = DARK.card;
pub const POPOVER: u32 = DARK.popover;
pub const DESTRUCTIVE: u32 = DARK.destructive;
pub const WARN: u32 = DARK.warn;
pub const SUCCESS: u32 = DARK.success;
