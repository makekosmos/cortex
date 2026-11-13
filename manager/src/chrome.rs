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
//! `HTMINBUTTON`/`HTMAXBUTTON`/`HTCLOSE` (and offers the maximize button's snap
//! layouts). The click itself is not Windows' though: `nsh-platform-gpui`
//! answers every press over its surface in-process (`events.rs`, the surface's
//! `MouseDown` handler ends with `cx.stop_propagation()`), and gpui only sets
//! `nc_button_pressed` when the press reaches Windows — so `DefWindowProc`
//! never gets the `WM_NCLBUTTONUP` it acts on, and a caption button that relies
//! on it is dead. The press is therefore taken here, in the capture phase, the
//! one place that runs before the surface can swallow it.
//!
//! The glyphs are Zeron's (`assets/icons/`): a 16-point box, 1.25 stroke, round
//! caps — thin and rounded, where NSH draws a square one-device-pixel line. A
//! maximized window gets the restore glyph, decided while painting, so it
//! changes in the frame the state does.
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

#[derive(Clone, Copy, PartialEq, Eq)]
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

/// Moving the window from a press, the way a title bar does — for a press on
/// the band, a sidebar's empty stretch or a page's heading row.
///
/// NSH's contract for `desktop={{ windowDrag: true }}` is the system's: on
/// Windows the area answers `HTCAPTION` and `DefWindowProc` moves, maximizes and
/// menus the window from there. Inside a host that never happens: the surface
/// marks the press handled (`nsh-platform-gpui`'s `chrome::drag_area` stops
/// propagation and then calls `Window::start_window_move`, whose Windows
/// implementation is the trait's empty default), so `DefWindowProc` never sees
/// it. So the pages ask for the move themselves (`ui/host.ts`), and it starts
/// here, where the window is: the capture is released and `WM_SYSCOMMAND` /
/// `SC_MOVE` is posted, which is how every custom title bar reaches Windows'
/// own move loop — the pointer follows, a maximized window restores under the
/// cursor, and Aero Snap works.
pub fn start_move(window: &mut gpui::Window, cx: &mut gpui::App) {
    #[cfg(target_os = "windows")]
    {
        let _ = cx;
        native::move_window(window);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = cx;
        window.start_window_move();
    }
}

/// Registers the caption buttons' presses. Painted with the shell, so it is
/// registered once per frame, as input listeners are.
#[cfg(target_os = "windows")]
pub fn input() -> impl IntoElement {
    gpui::canvas(|_, _, _| {}, |_, _, window, _| native::listen(window))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
}

#[cfg(not(target_os = "windows"))]
pub fn input() -> impl IntoElement {
    gpui::div()
}

#[cfg(target_os = "windows")]
mod native {
    use gpui::{px, DispatchPhase, MouseButton, MouseDownEvent, Pixels, Window};
    use std::cell::Cell;
    use std::rc::Rc;

    use super::{Control, BUTTON_WIDTH, TITLEBAR_HEIGHT};

    /// `HWND`, which is an opaque handle.
    type Hwnd = isize;

    const WM_SYSCOMMAND: u32 = 0x0112;
    /// `SC_MOVE`, with `HTCAPTION` in the low word as Windows expects it.
    const SC_MOVE: usize = 0xf010;
    const HTCAPTION: usize = 2;

    #[link(name = "user32")]
    extern "system" {
        fn ReleaseCapture() -> i32;
        fn PostMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
    }

    fn hwnd(window: &Window) -> Option<Hwnd> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
            _ => None,
        }
    }

    /// Hands the press to Windows' move loop, which then owns the pointer until
    /// the button comes up.
    pub(super) fn move_window(window: &Window) {
        let Some(hwnd) = hwnd(window) else { return };
        // SAFETY: a release of this window's capture and a posted system
        // command, both plain calls on this thread's own window.
        unsafe {
            ReleaseCapture();
            PostMessageW(hwnd, WM_SYSCOMMAND, SC_MOVE | HTCAPTION, 0);
        }
    }

    /// A left press on a caption button is the button's, and is answered here:
    /// `nsh-platform-gpui` ends its surface's `MouseDown` handler with
    /// `cx.stop_propagation()`, so gpui never gets as far as telling Windows
    /// which button was pressed and `DefWindowProc` never acts on the release.
    /// The press is taken in the capture phase, the one place that runs before
    /// the surface answers for it.
    pub(super) fn listen(window: &mut Window) {
        let answered: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
            if event.button != MouseButton::Left {
                return;
            }
            if phase == DispatchPhase::Capture {
                answered.set(false);
            }
            let Some(control) = button_at(event.position, window) else {
                return;
            };
            // Both phases see the press; the first one to answer does.
            if answered.replace(true) {
                return;
            }
            match control {
                Control::Minimize => window.minimize_window(),
                Control::Maximize => window.zoom_window(),
                Control::Close => window.remove_window(),
            }
        });
    }

    /// Which caption button a point is on, from the layout the pages keep clear
    /// for them: the strip is the rightmost `46 × 3` points of the band.
    fn button_at(position: gpui::Point<Pixels>, window: &Window) -> Option<Control> {
        let width = window.viewport_size().width;
        let (x, y) = (position.x, position.y);
        let strip = px(BUTTON_WIDTH * 3.);
        if y < px(0.) || y >= px(TITLEBAR_HEIGHT) || x < width - strip {
            return None;
        }
        match ((width - x) / px(BUTTON_WIDTH)) as u32 {
            0 => Some(Control::Close),
            1 => Some(Control::Maximize),
            _ => Some(Control::Minimize),
        }
    }
}
