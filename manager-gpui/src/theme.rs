//! Static dark palette — the "default" zeron theme from agenda-gpui's
//! generated palettes.rs (dark column), flattened for a single-mode shell.
#![allow(non_snake_case)]

use gpui::{rgb, Hsla};

pub const BG: u32 = 0x0d0c10;
pub const FG: u32 = 0xe7e5ea;
pub const MUTED_FG: u32 = 0x8d8a94;
pub const BORDER: u32 = 0x27262a;
pub const ACCENT: u32 = 0xaf94f9;
#[allow(dead_code)]
pub const ACCENT_FG: u32 = 0x0c0a0f;
#[allow(dead_code)]
pub const ACCENT_DIM: u32 = 0x826eb8;
#[allow(dead_code)]
pub const SECONDARY: u32 = 0x1c1b20;
pub const CARD: u32 = 0x131216;
pub const POPOVER: u32 = 0x18161c;
pub const SIDEBAR_BG: u32 = 0x09080c;
pub const SIDEBAR_DIVIDER: u32 = 0x1b1a1e;
pub const DESTRUCTIVE: u32 = 0xfa686a;
pub const WARN: u32 = 0xf4a25c;
pub const SUCCESS: u32 = 0x5bbd74;

pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn fade(hex: u32, a: f32) -> Hsla {
    let mut col: Hsla = rgb(hex).into();
    col.a = a;
    col
}
