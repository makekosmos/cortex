//! The dictation pill's dot scope, engine side. The simulation moved here
//! from the dictation app (`dot_scope/scope.rs` there) so the plugin is a
//! renderer: it receives the pose the scope settled on and one
//! (brightness, freshness) pair per dot, and draws them with its own shader.
//!
//! `pill_frame.rs` drives this at the display's cadence and broadcasts each
//! frame as `dictation_pill_frame`.

mod scope;
mod spring;
mod tuning;

pub use scope::{palette, Frame, Palette, Pose, Scope, Stage};
pub use tuning::{fixed_level, AutoLevel, Processing, Tuning};
