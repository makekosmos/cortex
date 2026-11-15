//! The pill window: the Engine's frames, painted.
//!
//! The Engine steps the dot scope and broadcasts each step; this window shows
//! the newest one with the app's shader and sits where the app's pill sat:
//! bottom center, 163 points up, 192×96, transparent, and it does not take
//! the pointer. It only exists while there is something to draw.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    div, img, point, px, size, App, AsyncApp, Bounds, ImageSource, RenderImage, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use image::Frame;
use serde_json::Value;

use crate::frames::Frames;
use crate::render::{Draw, Renderer, HEIGHT, WIDTH};

/// The app's established vertical center, independent of the shadow canvas.
const CENTER_FROM_BOTTOM: f32 = 163.;

/// How often the Engine is asked for a frame while the pill is on screen. The
/// scope advances by elapsed time, so this is the sampling rate, not the speed:
/// a 165 Hz display is fed at ~6 ms instead of the 16 ms a 60 Hz assumption
/// pinned it to (a slower ask showed on Windows as a visibly choppy pill).
const FRAME: Duration = Duration::from_millis(6);
/// Off screen nothing is drawn, so the frame slot is only checked for
/// something new. Short enough that the first frame of a session is not the
/// delay the person sees after the hotkey: an idle pill still costs ~40 wakeups
/// a second, which is nothing next to the window it belongs to.
const IDLE: Duration = Duration::from_millis(25);
/// How long the last frame stays on screen with no successor. The Engine ticks
/// every 6 ms while a session runs, so this only elapses when the session is
/// over (its fade ends the ticker) or the Engine went away.
const QUIET_HIDE: Duration = Duration::from_millis(400);

struct Plate {
    image: Option<Arc<RenderImage>>,
    /// The plate's size in whole points the window region was last shaped to:
    /// rounded up, so the region is never smaller than the plate it clips — a
    /// region a fraction of a point too small cuts the plate's soft edge off.
    shaped: (f32, f32),
}

impl Plate {
    fn set(&mut self, image: Arc<RenderImage>, plate: (f32, f32), window: &mut Window) {
        // A plate that comes back from hidden has no region left: the shape has to
        // be given to the window again, and the window has to be shown, not only
        // when the size changes.
        let returning = self.image.is_none();
        // Whole points, because the region is a hard rectangle: a fractional step
        // changes nothing on screen but does make DWM composite the window again.
        let shaped = (plate.0.ceil(), plate.1.ceil());
        #[cfg(target_os = "windows")]
        if self.shaped != shaped || returning {
            self.shaped = shaped;
            windows::shape(window, shaped);
        }
        #[cfg(target_os = "windows")]
        if returning {
            windows::show(window);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = returning;
            self.shaped = shaped;
        }
        if let Some(previous) = self.image.replace(image) {
            let _ = window.drop_image(previous);
        }
    }

    /// Back to nothing to draw: the image goes, and on Windows the region goes
    /// with it — a plate-less window there is DWM's milky rectangle.
    fn clear(&mut self, window: &mut Window) {
        if let Some(image) = self.image.take() {
            let _ = window.drop_image(image);
        }
        self.shaped = (0., 0.);
        #[cfg(target_os = "windows")]
        windows::hide(window);
    }
}

impl gpui::Render for Plate {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let plate = div().size_full();
        if let Some(image) = self.image.clone() {
            plate.child(img(ImageSource::Render(image)).size_full())
        } else {
            plate
        }
    }
}

fn num(value: &Value, key: &str) -> f32 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32
}

fn color4(value: &Value, key: &str, fallback: [f32; 4]) -> [f32; 4] {
    let Some(parts) = value.get(key).and_then(Value::as_array) else {
        return fallback;
    };
    let at = |index: usize, default: f32| {
        parts
            .get(index)
            .and_then(Value::as_f64)
            .map(|n| n as f32)
            .unwrap_or(default)
    };
    if parts.len() >= 4 {
        [
            at(0, fallback[0]),
            at(1, fallback[1]),
            at(2, fallback[2]),
            at(3, fallback[3]),
        ]
    } else {
        [
            at(0, fallback[0]),
            at(1, fallback[1]),
            at(2, fallback[2]),
            1.0,
        ]
    }
}

fn draw_of(frame: &Value) -> Option<Draw> {
    if frame.is_null() || !frame.is_object() {
        return None;
    }
    let opacity = num(frame, "opacity");
    if opacity < 0.02 {
        return None;
    }
    let dots = frame
        .get("dots")
        .and_then(Value::as_array)
        .map(|dots| {
            dots.chunks(2)
                .map(|pair| {
                    [
                        pair.first().and_then(Value::as_f64).unwrap_or(0.0) as f32,
                        pair.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    ]
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Draw {
        width: num(frame, "width"),
        height: num(frame, "height"),
        opacity,
        scale: num(frame, "scale").max(0.001),
        softness: num(frame, "softness"),
        glow: num(frame, "glow"),
        grid: num(frame, "grid"),
        live: num(frame, "live"),
        cool: num(frame, "cool"),
        columns: num(frame, "columns"),
        rows: num(frame, "rows"),
        base: color4(frame, "base", [0.86, 0.83, 0.77, 1.0]),
        core: color4(frame, "core", [0.98, 0.96, 0.92, 1.0]),
        plate: color4(frame, "plate", [0.067, 0.067, 0.078, 0.94]),
        rim: color4(frame, "rim", [1.0, 1.0, 1.0, 0.11]),
        shadow: color4(frame, "shadow", [0.0, 0.0, 0.0, 0.5]),
        dots,
    })
}

/// Where the pill writes what it did, when: `%LOCALAPPDATA%\Mundus\logs\dictation.log`.
/// A pill that appears late is measured against these lines, so each one carries
/// epoch milliseconds — the same clock a script or the Engine's own log uses.
pub(crate) fn log(what: &str) {
    use std::io::Write as _;
    let Some(dir) = std::env::var_os("LOCALAPPDATA").map(|local| PathBuf::from(local).join("Mundus\\logs")) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let ms = epoch_ms();
    let path = dir.join("dictation.log");
    // Two lines per dictation add up over months: past a quarter of a megabyte
    // the log starts again, which is all a delay report ever needs.
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > 256 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{ms} {what}");
    }
}

/// Epoch milliseconds: the clock a script and a frame dump share.
fn epoch_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default()
}

/// Watches the Engine's frames and draws them into a window that is created
/// once and kept: the process stays up between dictations, started by the
/// Engine, and its window only loses the plate when there is nothing to show.
pub fn start(cx: &mut App, frames: Frames) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let renderer = cx
            .background_executor()
            .spawn(async { Renderer::new() })
            .await;
        let Ok(mut renderer) = renderer else {
            eprintln!("dictation: GPU renderer did not start");
            return;
        };
        // Creating the window costs about a second (GPUI's first window on
        // Windows), and paying that after the hotkey was the delay the pill had:
        // the window is made here, blank and shapeless, and a session only fills
        // it and gives it a region. A different display under the pointer is the
        // one case that still opens a window.
        let mut placed = cx.update(|app| placed_on(app));
        let mut window = cx.update(open);
        let mut shown = false;
        // The last frame the Engine sent and when: a frame without a successor is
        // not the end of a session. `Frames::take` hands over whatever arrived
        // since the last look, so a tick of the Engine that lands a hair late
        // left the app with nothing to draw — and it hid the window, only to show
        // it again 30 ms later. That blink, seen on every start and every stage
        // change, reads as the window being re-created.
        let mut last_at: Option<std::time::Instant> = None;
        let dump = std::env::var_os("MUNDUS_PILL_DUMP").map(PathBuf::from);
        let mut shot = 0usize;
        loop {
            cx.background_executor()
                .timer(if shown { FRAME } else { IDLE })
                .await;
            let incoming = frames.take();
            let fresh = incoming.is_some();
            if fresh {
                last_at = Some(std::time::Instant::now());
            }
            // The picture stays while the Engine is only briefly quiet; a session
            // that really ended fades to nothing first, and this clock is the
            // backstop for one that ends without a fade.
            let quiet = last_at.is_none_or(|at| at.elapsed() > QUIET_HIDE);
            if !fresh && !quiet {
                continue;
            }
            let frame = incoming;
            let draw = frame.as_ref().and_then(draw_of);
            let buffer = draw
                .as_ref()
                .and_then(|draw| renderer.render_buffer(draw).ok());
            // `MUNDUS_PILL_DUMP=<dir>` writes every frame the plugin drew, so a
            // report about the picture can be checked against the picture.
            if let (Some(dir), Some(buffer)) = (dump.as_ref(), buffer.as_ref()) {
                shot += 1;
                if shot <= 400 {
                    let _ = buffer.save(dir.join(format!("frame-{shot:04}.png")));
                    if let Some(frame) = frame.as_ref() {
                        use std::io::Write as _;
                        if let Ok(mut file) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(dir.join("frames.txt"))
                        {
                            let _ = writeln!(file, "{shot} {} {}", epoch_ms(), frame);
                        }
                    }
                }
            }
            let image = buffer.map(|buffer| Arc::new(RenderImage::new(vec![Frame::new(buffer)])));
            let plate = draw
                .as_ref()
                .map_or((0., 0.), |draw| (draw.width * draw.scale, draw.height * draw.scale));
            if image.is_some() && !shown {
                let display = cx.update(|app| placed_on(app));
                if window.is_none() || (display.is_some() && display != placed) {
                    if window.is_some() {
                        close(&mut window, cx);
                    }
                    placed = display;
                    window = cx.update(open);
                }
            }
            let Some(handle) = window else {
                continue;
            };
            match image {
                Some(image) => {
                    let drawn = plate;
                    let _ = cx.update(|app| {
                        handle.update(app, |plate, window, cx| {
                            plate.set(image, drawn, window);
                            cx.notify();
                        })
                    });
                    if !shown {
                        shown = true;
                        log(&format!("pill shown {:.0}x{:.0}", drawn.0, drawn.1));
                    }
                }
                None if shown => {
                    shown = false;
                    let _ = cx.update(|app| handle.update(app, |plate, window, _| plate.clear(window)));
                    log("pill hidden");
                }
                None => {}
            }
        }
    })
    .detach();
}

/// Which display the pill belongs on: the one the pointer is on.
fn placed_on(cx: &App) -> Option<(f32, f32, f32, f32)> {
    let display = under_pointer(cx).or_else(|| cx.primary_display())?;
    let area = display.visible_bounds();
    Some((
        f32::from(area.origin.x),
        f32::from(area.origin.y),
        f32::from(area.size.width),
        f32::from(area.size.height),
    ))
}

/// The display the pointer is on, in the points GPUI lays its displays out in.
#[cfg(target_os = "macos")]
fn under_pointer(cx: &App) -> Option<std::rc::Rc<dyn gpui::PlatformDisplay>> {
    let (x, y) = unsafe { macos::pointer() }?;
    let inside = |display: &std::rc::Rc<dyn gpui::PlatformDisplay>| {
        let bounds = display.bounds();
        let (left, top) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        x >= left && x <= left + width && y >= top && y <= top + height
    };
    cx.displays().into_iter().find(inside)
}

#[cfg(not(target_os = "macos"))]
fn under_pointer(_cx: &App) -> Option<std::rc::Rc<dyn gpui::PlatformDisplay>> {
    None
}

fn open(cx: &mut App) -> Option<gpui::WindowHandle<Plate>> {
    // The person dictates where they work: a second screen gets its own pill
    // instead of one that opens on the main display.
    let display = under_pointer(cx).or_else(|| cx.primary_display())?;
    let area = display.visible_bounds();
    let width = WIDTH as f32;
    let height = HEIGHT as f32;
    let origin = point(
        area.center().x - px(width / 2.),
        area.origin.y + area.size.height - px(CENTER_FROM_BOTTOM + height / 2.),
    );
    let bounds = Bounds {
        origin,
        size: size(px(width), px(height)),
    };
    let handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: false,
                show: true,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            move |window, cx| {
                overlay(window);
                #[cfg(target_os = "windows")]
                windows::hide(window);
                cx.new(|_| Plate {
                    image: None,
                    shaped: (0., 0.),
                })
            },
        )
        .ok()?;
    log("pill window created");
    Some(handle)
}

fn close(window: &mut Option<gpui::WindowHandle<Plate>>, cx: &mut AsyncApp) {
    let Some(handle) = window.take() else {
        return;
    };
    let _ = cx.update(|app| {
        handle.update(app, |plate, window, _| {
            plate.clear(window);
            window.remove_window();
        })
    });
    log("pill window closed");
}

/// A HUD, not a window: no pointer, no shadow, above the Dock, on every space.
fn overlay(window: &mut Window) {
    window.set_background_appearance(WindowBackgroundAppearance::Transparent);
    window.set_input_region(Some(&[]));
    #[cfg(target_os = "macos")]
    macos::overlay(window);
}

/// A "transparent" window on Windows is not transparent to DWM: it paints its
/// own backdrop across the whole HUD, which is the milky rectangle that used to
/// hang around the pill. The renderer already draws the pill with per-pixel
/// alpha, so the window is simply shaped to the plate — a stadium as wide as
/// the plate plus room for its shadow, where DWM has nothing left to paint.
#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::c_void;

    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use crate::render::{HEIGHT, WIDTH};

    /// Room around the plate for the shader's drop shadow. 18 pt out the
    /// shadow is already below 1/255, so the cut cannot be seen.
    const MARGIN: f32 = 18.0;

    fn hwnd(window: &Window) -> Option<*mut c_void> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return None;
        };
        Some(win32.hwnd.get() as *mut c_void)
    }

    /// Nothing to draw: an empty region, which is no visible area and no
    /// hit-testing, and the window itself hidden.
    pub fn hide(window: &Window) {
        let Some(hwnd) = hwnd(window) else {
            return;
        };
        unsafe {
            let region = CreateRectRgn(0, 0, 0, 0);
            if region != 0 && SetWindowRgn(hwnd, region, 1) == 0 {
                DeleteObject(region);
            }
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    /// Back on screen without taking the focus: `SW_SHOWNA` is "show, do not
    /// activate" — the pill must never pull the person out of what they type in.
    pub fn show(window: &Window) {
        let Some(hwnd) = hwnd(window) else {
            return;
        };
        unsafe {
            ShowWindow(hwnd, SW_SHOWNA);
        }
    }

    pub fn shape(window: &Window, plate: (f32, f32)) {
        let Some(hwnd) = hwnd(window) else {
            return;
        };
        if plate.0 < 1.0 || plate.1 < 1.0 {
            return;
        }
        let width = plate.0 + MARGIN * 2.0;
        let height = plate.1 + MARGIN * 2.0;
        let left = ((WIDTH as f32 - width) / 2.0).round();
        let top = ((HEIGHT as f32 - height) / 2.0).round();
        let (right, bottom) = (left + width.round(), top + height.round());
        if std::env::var_os("MUNDUS_PILL_DUMP").is_some() {
            super::log(&format!(
                "shape {:.0}x{:.0} rgn {left:.0},{top:.0},{right:.0},{bottom:.0}",
                plate.0, plate.1
            ));
        }
        let corner = height.round();
        unsafe {
            let region = CreateRoundRectRgn(
                left as i32,
                top as i32,
                right as i32,
                bottom as i32,
                corner as i32,
                corner as i32,
            );
            if region == 0 {
                return;
            }
            // Windows owns the region once it is set; only a failure frees it.
            if SetWindowRgn(hwnd, region, 1) == 0 {
                DeleteObject(region);
            }
        }
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowRgn(window: *mut c_void, region: isize, redraw: i32) -> i32;
        fn ShowWindow(window: *mut c_void, command: i32) -> i32;
    }

    /// Hide with no animation and no activation bookkeeping.
    const SW_HIDE: i32 = 0;
    /// Show without activating: the pill never takes the person's focus.
    const SW_SHOWNA: i32 = 8;

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> isize;
        fn CreateRoundRectRgn(
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
            width: i32,
            height: i32,
        ) -> isize;
        fn DeleteObject(object: isize) -> i32;
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;

    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn overlay(window: &Window) {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::AppKit(view) = handle.as_raw() else {
            return;
        };
        unsafe {
            let view = view.ns_view.as_ptr();
            let ns_window = msg0(view, c"window".as_ptr());
            if ns_window.is_null() {
                return;
            }
            msg_bool(ns_window, c"setIgnoresMouseEvents:".as_ptr(), true);
            msg_bool(ns_window, c"setHasShadow:".as_ptr(), false);
            msg_bool(ns_window, c"setOpaque:".as_ptr(), false);
            msg_bool(ns_window, c"setHidesOnDeactivate:".as_ptr(), false);
            // Above the Dock, and still there when another app is in front.
            msg_i64(ns_window, c"setLevel:".as_ptr(), 25);
            msg_u64(ns_window, c"setCollectionBehavior:".as_ptr(), 1 | 16 | 256);
            msg0(ns_window, c"orderFrontRegardless".as_ptr());
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGPoint {
        pub x: f64,
        pub y: f64,
    }

    /// The pointer's place in the screen coordinates CoreGraphics reports, the
    /// same ones GPUI lays its displays out in.
    pub unsafe fn pointer() -> Option<(f32, f32)> {
        let event = CGEventCreate(std::ptr::null_mut());
        if event.is_null() {
            return None;
        }
        let place = CGEventGetLocation(event);
        CFRelease(event);
        Some((place.x as f32, place.y as f32))
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: *mut c_void) -> *mut c_void;
        fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(value: *const c_void);
    }

    unsafe fn msg0(obj: *mut c_void, sel: *const i8) -> *mut c_void {
        let sel = sel_registerName(sel);
        let call: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        call(obj, sel)
    }

    unsafe fn msg_bool(obj: *mut c_void, sel: *const i8, value: bool) {
        let sel = sel_registerName(sel);
        let call: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) =
            std::mem::transmute(objc_msgSend as *const ());
        call(obj, sel, value);
    }

    unsafe fn msg_i64(obj: *mut c_void, sel: *const i8, value: i64) {
        let sel = sel_registerName(sel);
        let call: unsafe extern "C" fn(*mut c_void, *mut c_void, i64) =
            std::mem::transmute(objc_msgSend as *const ());
        call(obj, sel, value);
    }

    unsafe fn msg_u64(obj: *mut c_void, sel: *const i8, value: u64) {
        let sel = sel_registerName(sel);
        let call: unsafe extern "C" fn(*mut c_void, *mut c_void, u64) =
            std::mem::transmute(objc_msgSend as *const ());
        call(obj, sel, value);
    }

    #[link(name = "objc")]
    extern "C" {
        fn sel_registerName(name: *const i8) -> *mut c_void;
        fn objc_msgSend();
    }
}
