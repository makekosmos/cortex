use crate::views::View;
use gpui::{px, size, TestAppContext};

#[gpui::test]
fn all_pages_share_centered_width_with_sidebar_open_or_closed(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    cx.simulate_resize(size(px(1440.), px(900.)));
    for open in [true, false] {
        for view in [
            View::Data,
            View::Usage,
            View::Sync,
            View::Packages,
            View::Engine,
            View::Settings,
            View::Connections,
            View::About,
            View::Updates,
            View::Secrets,
            View::Browser,
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
            assert!(
                f32::from(page.size.width) <= 760.1,
                "page too wide: {view:?}"
            );
            let delta = f32::from(page.center().x - content.center().x).abs();
            assert!(
                delta <= 1.0,
                "page not centered: {view:?}, sidebar={open}, delta={delta}"
            );
        }
    }
}
