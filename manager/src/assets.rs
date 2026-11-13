use std::borrow::Cow;

/// The design's own typeface: the pages ask for `Inter`, so the host registers
/// it as Atlas does, and the fallback never shows.
pub fn font_bytes() -> Vec<Cow<'static, [u8]>> {
    vec![Cow::Borrowed(
        include_bytes!("../assets/fonts/Inter.ttf") as &[u8]
    )]
}
