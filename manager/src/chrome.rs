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
//! maximize button's snap layouts). The glyphs are Zeron's (`assets/icons/`):
//! a 16-point box, 1.25 stroke, round caps — thin and rounded, where NSH draws
//! a square one-device-pixel line. A maximized window gets the restore glyph,
//! decided while painting, so it changes in the frame the state does.
//!
//! Only Windows: on macOS AppKit keeps the traffic lights, and on Linux the
//! window manager decides.

use gpui::{
    canvas, div, prelude::*, px, Rgba, SharedString, TransformationMatrix, WindowControlArea,
};

/// The strip `setTitleBar` takes for the app's own bar; the pages' heading row
/// is laid out inside it.
pub const TITLEBAR_HEIGHT: f32 = 36.;
/// A caption button's width, and the three together are the `138` points
/// `titleBar()` reserves while NSH owns the window.
const BUTTON_WIDTH: f32 = 46.;
/// The glyph's box: the SVGs' own 16-point viewBox, so their 6.5-point shapes
/// land at the size Zeron's titlebar draws them.
const GLYPH: f32 = 16.;

/// Zeron's caption glyphs, verbatim (`assets/icons/README.md` says where they
/// come from and under which licence).
const MINIMIZE: &[u8] = include_bytes!("../assets/icons/window-minimize.svg");
const MAXIMIZE: &[u8] = include_bytes!("../assets/icons/window-maximize.svg");
const RESTORE: &[u8] = include_bytes!("../assets/icons/window-restore.svg");
const CLOSE: &[u8] = include_bytes!("../assets/icons/close.svg");

#[derive(Clone, Copy)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

#[derive(Clone, Copy)]
struct Colors {
    glyph: Rgba,
    /// The glyphs of a window that is not the active one — Windows dims them
    /// the same way, so an unfocused Manager reads as unfocused.
    glyph_idle: Rgba,
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
                glyph_idle: gpui::rgba(0xffffff80),
                hover: gpui::rgba(0xffffff14),
                press: gpui::rgba(0xffffff24),
            }
        } else {
            Self {
                glyph: gpui::rgba(0x000000e6),
                glyph_idle: gpui::rgba(0x00000080),
                hover: gpui::rgba(0x00000012),
                press: gpui::rgba(0x00000020),
            }
        }
    }
}

/// One glyph, as its SVG: `paint_svg` rasterizes the file's own geometry
/// (round caps, rounded corners) at the element's box, and the maximized state
/// is read while painting — a window that is maximized as the frame is drawn
/// shows the restore shape, not the shape it had when the pages laid out.
fn glyph(control: Control, colors: Colors) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let (name, data) = match control {
                Control::Minimize => ("caption-minimize", MINIMIZE),
                Control::Maximize if window.is_maximized() => ("caption-restore", RESTORE),
                Control::Maximize => ("caption-maximize", MAXIMIZE),
                Control::Close => ("caption-close", CLOSE),
            };
            let color = if window.is_window_active() {
                colors.glyph
            } else {
                colors.glyph_idle
            };
            window
                .paint_svg(
                    bounds,
                    SharedString::from(name),
                    Some(data),
                    TransformationMatrix::default(),
                    color.into(),
                    cx,
                )
                .ok();
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
        .child(glyph(control, colors))
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
