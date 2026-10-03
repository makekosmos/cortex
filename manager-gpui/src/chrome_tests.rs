use super::{content_inset, drag_inset, toggle_left};
use imago_gpui::chrome::SIDEBAR_W;

#[test]
fn macos_toggle_clears_traffic_lights_and_reclaims_fullscreen_space() {
    assert_eq!(toggle_left(true, false), 88.0);
    assert_eq!(toggle_left(true, true), 12.0);
    assert_eq!(toggle_left(false, false), 12.0);
    assert_eq!(toggle_left(false, true), 12.0);
}

#[test]
fn title_and_drag_region_clear_toggle_throughout_sidebar_animation() {
    for is_macos in [false, true] {
        for fullscreen in [false, true] {
            for step in 0..=100 {
                let progress = step as f32 / 100.0;
                let global_start =
                    SIDEBAR_W * progress + content_inset(progress, is_macos, fullscreen);
                assert!(global_start >= drag_inset(is_macos, fullscreen));
            }
            assert_eq!(content_inset(1.0, is_macos, fullscreen), 11.0);
            assert_eq!(
                content_inset(0.0, is_macos, fullscreen),
                drag_inset(is_macos, fullscreen)
            );
        }
    }
}
