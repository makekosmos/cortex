use crate::views::View;
use gpui::{px, size, TestAppContext};

#[gpui::test]
fn all_pages_share_centered_width_with_sidebar_open_or_closed(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    for width in [900., 1440.] {
        cx.simulate_resize(size(px(width), px(900.)));
        for open in [true, false] {
            for view in [
                View::Data,
                View::Usage,
                View::Sync,
                View::Packages,
                View::Settings,
                View::Appearance,
                View::Browser,
                View::Connections,
                View::Keys,
                View::About,
                View::Dev,
            ] {
                manager.update(cx, |app, cx| {
                    app.view = view;
                    app.sidebar_t = if open { 1.0 } else { 0.0 };
                    app.sidebar_target = app.sidebar_t;
                    cx.notify();
                });
                cx.update(|_, cx| cx.refresh_windows());
                let page = cx
                    .debug_bounds("page-column")
                    .expect("shared page container");
                let content = cx.debug_bounds("page-content").expect("content pane");
                if view != View::About {
                    let heading = cx.debug_bounds("page-heading").expect("shared H1");
                    let body = cx.debug_bounds("page-body").expect("shared body");
                    assert_eq!(heading.top(), page.top(), "extra top inset: {view:?}");
                    assert_eq!(
                        heading.size.height,
                        px(20.),
                        "H1 must not have a description: {view:?}"
                    );
                    assert_eq!(
                        body.top() - heading.bottom(),
                        px(24.),
                        "inconsistent content inset: {view:?}"
                    );
                    assert_eq!(
                        body.left(),
                        heading.left(),
                        "inconsistent left inset: {view:?}"
                    );
                }
                assert!(
                    f32::from(page.size.width) <= 760.1,
                    "page too wide: {view:?}"
                );
                assert!(
                    f32::from(page.size.width) <= f32::from(content.size.width) - 47.,
                    "page exceeds padded viewport: {view:?}, width={width}"
                );
                let delta = f32::from(page.center().x - content.center().x).abs();
                assert!(
                    delta <= 1.0,
                    "page not centered: {:?}, sidebar={}, delta={}",
                    view,
                    open,
                    delta
                );
            }
        }
    }
}
