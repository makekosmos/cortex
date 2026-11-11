//! The dictation pill, the same picture the app drew.
//!
//! The Engine steps the dot scope and keeps the latest frame. This window
//! paints that frame with the app's shader and sits where the app's pill sat:
//! bottom center, 163 points up, 192×96, transparent, and it does not take
//! the pointer.

use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    div, img, point, px, size, App, AsyncApp, Bounds, ImageSource, RenderImage, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use serde_json::Value;

use crate::engine::Engine;
use crate::gpu_scope::{Draw, Renderer, HEIGHT, WIDTH};

/// The app's established vertical center, independent of the shadow canvas.
const CENTER_FROM_BOTTOM: f32 = 163.;

struct Plate {
    image: Option<Arc<RenderImage>>,
}

impl Plate {
    fn set(&mut self, image: Arc<RenderImage>, window: &mut Window) {
        if let Some(previous) = self.image.replace(image) {
            let _ = window.drop_image(previous);
        }
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

fn fetch(engine: &Engine) -> Option<Draw> {
    let value = engine
        .rpc("dictation.pill_frame", serde_json::json!({}))
        .ok()?;
    if value.get("active").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    draw_of(value.get("frame").unwrap_or(&Value::Null))
}

/// Watches the Engine and shows the pill while a frame has something to draw.
pub fn start(cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let engine = Engine::new(None);
        let renderer = cx
            .background_executor()
            .spawn(async { Renderer::new() })
            .await;
        let Ok(mut renderer) = renderer else {
            eprintln!("pill: GPU renderer did not start");
            return;
        };
        let mut window: Option<gpui::WindowHandle<Plate>> = None;
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(33))
                .await;
            let engine = engine.clone();
            let draw = cx
                .background_executor()
                .spawn(async move { fetch(&engine) })
                .await;
            let image = draw.as_ref().and_then(|draw| renderer.render(draw).ok());
            cx.update(|app| match image {
                Some(image) => match window {
                    Some(handle) => {
                        let _ = handle.update(app, |plate, window, cx| {
                            plate.set(image, window);
                            cx.notify();
                        });
                    }
                    None => window = open(app, image),
                },
                None => close(&mut window, app),
            });
        }
    })
    .detach();
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

fn open(cx: &mut App, image: Arc<RenderImage>) -> Option<gpui::WindowHandle<Plate>> {
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
                cx.new(|_| Plate { image: Some(image) })
            },
        )
        .ok()?;
    Some(handle)
}

fn close(window: &mut Option<gpui::WindowHandle<Plate>>, cx: &mut App) {
    let Some(handle) = window.take() else {
        return;
    };
    let _ = handle.update(cx, |plate, window, _| {
        if let Some(image) = plate.image.take() {
            let _ = window.drop_image(image);
        }
        window.remove_window();
    });
}

/// A HUD, not a window: no pointer, no shadow, above the Dock, on every space.
fn overlay(window: &mut Window) {
    window.set_background_appearance(WindowBackgroundAppearance::Transparent);
    window.set_input_region(Some(&[]));
    #[cfg(target_os = "macos")]
    macos::overlay(window);
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
