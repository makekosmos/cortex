//! Dictation — the pill.
//!
//! The Engine owns the hotkey, the recording, the transcription and the
//! insertion; it also owns the dot scope, and it broadcasts every step as
//! `dictation_pill_frame`. This process is the picture: it subscribes to those
//! frames and paints them in a 192×96 window that takes no pointer, sits above
//! other windows, and is absent whenever there is nothing to draw.
//!
//! Nothing of the product is required for a dictation to work: with this
//! process gone the Engine still records, transcribes and inserts.

mod frames;
mod hud;
mod render;

use frames::Frames;

fn main() {
    let frames = Frames::start();
    gpui::application().run(move |cx: &mut gpui::App| {
        hud::start(cx, frames.clone());
    });
}
