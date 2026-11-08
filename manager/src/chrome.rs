//! The window's caption buttons, for the window this app owns.
//!
//! NSH draws them only when it owns the window — under `nsh dev` it reports the
//! strip in `titleBar()` and paints `46 × 3` points of it itself. Inside a host
//! it reports nothing reserved and leaves that strip to the app, which is why
//! the pages cannot do this job: a button the pages draw is an ordinary client
//! view, and Windows hit-tests it as `HTCLIENT`. Worse, the pages' own drag
//! area covers the strip, so a press there starts a window move and the button
//! never sees it.
//!
//! So the three buttons live here, next to the window, and mark their areas
//! with [`WindowControlArea`] the same way NSH does — Windows then answers
//! `HTMINBUTTON`/`HTMAXBUTTON`/`HTCLOSE`, handles the clicks itself (and the
//! maximize button's snap layouts). The glyphs are NSH's: one device pixel,
//! square corners, the shapes Windows uses.
//!
//! Only Windows: on macOS AppKit keeps the traffic lights, and on Linux the
//! window manager decides.

use gpui::{canvas, div, prelude::*, px, PathBuilder, Rgba, WindowControlArea};

/// The strip `setTitleBar` takes for the app's own bar; the pages' heading row
/// is laid out inside it.
pub const TITLEBAR_HEIGHT: f32 = 36.;
/// A caption button's width, and the three together are the `138` points
/// `titleBar()` reserves while NSH owns the window.
const BUTTON_WIDTH: f32 = 46.;
/// The glyph's box, in points, as NSH draws it.
const GLYPH: f32 = 10.;

#[derive(Clone, Copy)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

#[derive(Clone, Copy)]
struct Colors {
    glyph: Rgba,
    hover: Rgba,
    press: Rgba,
}

impl Colors {
    /// The caption's colors follow the system's appearance, not the Engine's
    /// theme: the buttons belong to the window frame, and Windows draws its own
    /// the same way over a light or a dark app.
    fn for_appearance(dark: bool) -> Self {
        if dark {
            Self {
                glyph: gpui::rgba(0xffffffe6),
                hover: gpui::rgba(0xffffff14),
                press: gpui::rgba(0xffffff24),
            }
        } else {
            Self {
                glyph: gpui::rgba(0x000000e6),
                hover: gpui::rgba(0x00000012),
                press: gpui::rgba(0x00000020),
            }
        }
    }
}

/// The glyph's path, at a device pixel wide whatever the display's scale is —
/// the same geometry `nsh-platform-gpui` draws, so the Manager's window looks
/// like the one `nsh dev` opens.
fn glyph(control: Control, color: Rgba) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            let line = 1.0_f32 / window.scale_factor();
            let p = |x: f32, y: f32| gpui::point(px(x0 + x), px(y0 + y));
            let mut path = PathBuilder::stroke(px(line.max(1.0)));
            match control {
                Control::Minimize => {
                    path.move_to(p(0., GLYPH / 2.));
                    path.line_to(p(GLYPH, GLYPH / 2.));
                }
                Control::Maximize => {
                    path.move_to(p(0., 0.));
                    path.line_to(p(GLYPH, 0.));
                    path.line_to(p(GLYPH, GLYPH));
                    path.line_to(p(0., GLYPH));
                    path.close();
                }
                Control::Close => {
                    path.move_to(p(0., 0.));
                    path.line_to(p(GLYPH, GLYPH));
                    path.move_to(p(GLYPH, 0.));
                    path.line_to(p(0., GLYPH));
                }
            }
            let path = path.build().expect("caption glyph path");
            window.paint_path(path, color);
        },
    )
    .w(px(GLYPH))
    .h(px(GLYPH))
}

fn button(
    id: &'static str,
    control: Control,
    area: WindowControlArea,
    area_right: f32,
    colors: Colors,
) -> impl IntoElement {
    div()
        .id(id)
        .absolute()
        .top_0()
        .right(px(area_right))
        .w(px(BUTTON_WIDTH))
        .h(px(TITLEBAR_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        .hover(move |style| style.bg(colors.hover))
        .active(move |style| style.bg(colors.press))
        .child(glyph(control, colors.glyph))
}

/// The three caption buttons, right to left: close, maximize, minimize — the
/// order Windows puts them in, over the strip the pages leave for them.
pub fn controls(dark: bool) -> Option<impl IntoElement> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    let colors = Colors::for_appearance(dark);
    Some(
        div()
            .absolute()
            .top_0()
            .right_0()
            .flex()
            .child(button(
                "caption-min",
                Control::Minimize,
                WindowControlArea::Min,
                BUTTON_WIDTH * 2.,
                colors,
            ))
            .child(button(
                "caption-max",
                Control::Maximize,
                WindowControlArea::Max,
                BUTTON_WIDTH,
                colors,
            ))
            .child(button(
                "caption-close",
                Control::Close,
                WindowControlArea::Close,
                0.,
                colors,
            )),
    )
}

/// Moving the window from a press, the way a title bar does.
///
/// NSH's contract for `desktop={{ windowDrag: true }}` is the system's: on
/// Windows the area answers `HTCAPTION` and `DefWindowProc` moves, maximizes and
/// menus the window from there. Inside a host that never happens: the surface
/// marks the press handled (`nsh-platform-gpui`'s `chrome::drag_area` stops
/// propagation and then calls `Window::start_window_move`, which the Windows
/// backend does not implement), so `DefWindowProc` never sees it. Measured on
/// this window: a `HTCAPTION` press comes back handled in a few milliseconds,
/// where a real move would keep it for as long as the drag lasts.
///
/// So the window starts the move itself. Which press counts is still the
/// platform's own answer — the surface marks drag areas `HTCAPTION` and carves
/// buttons, fields and rows out of them — asked a moment later, because that
/// answer comes from the app's hit test and the app is busy dispatching the
/// press while it is being dispatched.
pub fn start_move(window: &mut gpui::Window, cx: &mut gpui::App) {
    #[cfg(target_os = "windows")]
    {
        let async_cx = window.to_async(cx);
        async_cx
            .spawn(async move |cx: &mut gpui::AsyncWindowContext| {
                let background = cx.background_executor().clone();
                background.timer(std::time::Duration::from_millis(1)).await;
                cx.update(|window, _| drag::move_now(window)).ok();
            })
            .detach();
    }
    #[cfg(not(target_os = "windows"))]
    window.start_window_move();
}

/// Registers the press listener [`start_move`] works from. Painted with the
/// shell, so it is registered once per frame, as input listeners are.
#[cfg(target_os = "windows")]
pub fn drag() -> impl IntoElement {
    gpui::canvas(|_, _, _| {}, |_, _, window, _| drag::listen(window))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
}

#[cfg(not(target_os = "windows"))]
pub fn drag() -> impl IntoElement {
    gpui::div()
}

#[cfg(target_os = "windows")]
mod drag {
    use gpui::{App, DispatchPhase, MouseButton, MouseDownEvent, Window};

    /// `HWND`, which is an opaque handle.
    type Hwnd = isize;

    const WM_NCHITTEST: u32 = 0x0084;
    const WM_SYSCOMMAND: u32 = 0x0112;
    const HTCAPTION: isize = 2;
    const SC_MOVE: usize = 0xF010;
    const VK_LBUTTON: i32 = 0x01;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
        fn PostMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
        fn ReleaseCapture() -> i32;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn GetAsyncKeyState(key: i32) -> i16;
    }

    fn packed(point: Point) -> isize {
        ((point.y as isize) << 16) | (point.x as isize & 0xffff)
    }

    fn hwnd(window: &Window) -> Option<Hwnd> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
            _ => None,
        }
    }

    /// Start the move when the press under the cursor is one a title bar takes.
    pub(super) fn move_now(window: &Window) {
        let Some(hwnd) = hwnd(window) else { return };
        let mut point = Point::default();
        // SAFETY: plain Win32 reads and the message a title bar press sends,
        // on this thread and this thread's own window.
        unsafe {
            GetCursorPos(&mut point);
            let at = packed(point);
            if SendMessageW(hwnd, WM_NCHITTEST, 0, at) != HTCAPTION {
                return;
            }
            // A press that is over by now (a quick click) would leave a move
            // loop waiting for the next click, so the button must still be down.
            if GetAsyncKeyState(VK_LBUTTON) >= 0 {
                return;
            }
            ReleaseCapture();
            // Posted rather than sent: the loop pumps messages, and it must not
            // run from inside NSH's own dispatch.
            PostMessageW(hwnd, WM_SYSCOMMAND, SC_MOVE, 0);
        }
    }

    /// Every left press is offered; the hit test decides, so a press on a row, a
    /// field or a caption button never moves anything.
    pub(super) fn listen(window: &mut Window) {
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx: &mut App| {
            if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                return;
            }
            super::start_move(window, cx);
        });
    }
}
