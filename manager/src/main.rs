//! Mundus Manager: a GPUI window whose pages are NSH (TSX), the same stack
//! Atlas runs — gpui-kit 0.8 on gpui-fast, `imago_nsh::AppView` hosting the
//! bundle, and `manager.bridge` connecting the pages to the Engine.

#![windows_subsystem = "windows"]

mod assets;
mod boot;
mod bridge;
mod chrome;
mod data;
mod engine;
mod gpu_scope;
mod hotkey_view;
mod identity;
mod lifecycle;
mod log;
mod menu;
mod pill;
mod plugins;

use gpui::prelude::*;
use gpui::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};
#[cfg(target_os = "windows")]
use gpui_fast_backend as _;
use imago_nsh::{Bundle, NshApp};

const WINDOW_TITLE: &str = identity::PRODUCT_NAME;
// A control panel, not a workspace: a small window is enough, and the person
// resizes it when a page wants more room.
const WINDOW_WIDTH: f32 = 800.;
const WINDOW_HEIGHT: f32 = 600.;

/// The window's content: the NSH surface inside gpui-component's Root, with the
/// window's own caption buttons over it.
struct Shell {
    root: gpui::Entity<gpui_component::Root>,
}

impl gpui::Render for Shell {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let dark = matches!(
            window.appearance(),
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        );
        gpui::div()
            .size_full()
            .relative()
            .child(self.root.clone())
            .child(chrome::input())
            .children(chrome::controls(dark))
    }
}

/// `MUNDUS_STARTUP_TRACE=1`: how long each step of opening the window took,
/// on stderr. Off by default.
fn boot_trace(boot: &std::time::Instant, label: &str) {
    if std::env::var_os("MUNDUS_STARTUP_TRACE").is_none() {
        return;
    }
    let since_spawn = ms_since_process_start()
        .map(|ms| format!(", {ms} ms since the process started"))
        .unwrap_or_default();
    eprintln!(
        "boot: {label} ({} ms in main{since_spawn})",
        boot.elapsed().as_millis()
    );
}

/// Time from process creation to now: includes mapping the exe and loading its
/// DLLs, which `Instant` cannot see.
#[cfg(windows)]
fn ms_since_process_start() -> Option<u128> {
    use std::ffi::c_void;
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessTimes(
            process: *mut c_void,
            creation: *mut u64,
            exit: *mut u64,
            kernel: *mut u64,
            user: *mut u64,
        ) -> i32;
        fn GetSystemTimeAsFileTime(time: *mut u64);
    }
    unsafe {
        let mut creation = 0u64;
        let (mut exit, mut kernel, mut user) = (0u64, 0u64, 0u64);
        if GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        ) == 0
        {
            return None;
        }
        let mut now = 0u64;
        GetSystemTimeAsFileTime(&mut now);
        Some(u128::from(now.saturating_sub(creation)) / 10_000)
    }
}

#[cfg(not(windows))]
fn ms_since_process_start() -> Option<u128> {
    None
}

/// Frames 1..34 with their timings: the JS bundle is evaluated on the first
/// frames, so "opened" can mean "the first frame" or "the frame with the real
/// pages".
fn arm_frame_trace(
    window: &mut gpui::Window,
    boot: std::time::Instant,
    seen: std::rc::Rc<std::cell::Cell<u32>>,
) {
    window.on_next_frame(move |window, _cx| {
        let frame = seen.get() + 1;
        seen.set(frame);
        if matches!(frame, 1 | 2 | 3 | 5 | 8 | 13 | 21 | 34) {
            boot_trace(&boot, &format!("frame {frame}"));
        }
        if frame < 34 {
            arm_frame_trace(window, boot, seen.clone());
        }
    });
}

fn main() {
    let boot = std::time::Instant::now();
    // The pages only mirror the Engine, so one has to answer before the window
    // opens. `--check-engine` runs that wait alone, for the installer smoke
    // test; `MUNDUS_SKIP_ENGINE_BOOT=1` leaves the lifetime to whoever started
    // this process (dev runs against an Engine already up).
    if std::env::var("MUNDUS_SKIP_ENGINE_BOOT").is_err() {
        if let Err(error) = boot::ensure_engine() {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
    boot_trace(&boot, "engine ready");
    if std::env::args().any(|argument| argument == "--check-engine") {
        println!("Engine ready");
        return;
    }
    let application = gpui::application();
    boot_trace(&boot, "gpui object");
    application.run(move |cx: &mut App| {
        boot_trace(&boot, "app.run");
        gpui_component::init(cx);
        // The pages are embedded in this window, not in an NSH surface of their
        // own, so the menu bar macOS expects is placed here; without it Cmd+Q
        // answers to nothing.
        menu::install(cx);
        cx.text_system()
            .add_fonts(assets::font_bytes())
            .expect("failed to load the design's fonts");
        boot_trace(&boot, "fonts");
        // `MUNDUS_MANAGER_OFFSCREEN=1` parks the window off the desktop so a
        // soak run does not pop up over the user's work.
        let bounds = if std::env::var("MUNDUS_MANAGER_OFFSCREEN").is_ok() {
            gpui::bounds(
                gpui::point(px(-20000.), px(-20000.)),
                size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
            )
        } else {
            Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx)
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // Same as Atlas (KOS-368): a title at creation reaches AppKit's
            // `changeWindowsItem:` before the window is tracked, which leaves a
            // phantom Dock entry and turns the real window's close, minimize
            // and zoom into something other than the system's. `set_window_title`
            // below renames the entry AppKit added itself.
            titlebar: Some(gpui::TitlebarOptions {
                title: None,
                appears_transparent: true,
                // Down and in from the corner, the way Chrome and System
                // Settings place them: AppKit's default hugs the very corner
                // and lands on the sidebar's search field. The sidebar keeps
                // `MAC_LIGHTS_INSET` clear above its own content.
                traffic_light_position: Some(gpui::point(px(20.), px(18.))),
            }),
            // The pages drag the window themselves (`desktop={{ windowDrag }}`)
            // from the page heading and the sidebar, so AppKit must not drag the
            // same strip a second time.
            app_owns_titlebar_drag: true,
            ..Default::default()
        };
        // A second launch (the tray's «Открыть интерфейс») belongs to the
        // process already drawing the pill: reveal its window and leave.
        // `MUNDUS_TRACE_TITLE` only exists so a soak run can open its own window
        // next to the running one instead of revealing it.
        let title = std::env::var("MUNDUS_TRACE_TITLE").unwrap_or_else(|_| WINDOW_TITLE.to_string());
        if lifecycle::reveal_existing(&title) {
            std::process::exit(0);
        }
        pill::start(cx);
        boot_trace(&boot, "pill+hotkey");
        let opened = cx.open_window(options, |window, cx| {
            window.set_window_title(title.as_str());
            boot_trace(&boot, "window object");
            let view = cx.new(|cx| {
                let view =
                    imago_nsh::AppView::new(nsh_platform_gpui::AppOptions::default(), window, cx);
                view.register_component(
                    "manager.bridge",
                    std::rc::Rc::new(|context| Box::new(bridge::ManagerBridge::new(context))),
                );
                view.on_error(|message| eprintln!("Manager nsh: {message}"));
                // Atlas passes its first snapshot as params; the Manager sends
                // the same state as a `state` event, so an empty map is enough —
                // but it must exist, because the pages read `host.params`.
                view.host().set_params(serde_json::Map::new());
                if let Ok(url) = std::env::var("MUNDUS_NSH_DEV_SERVER") {
                    view.connect_dev_server(url, cx);
                } else {
                    view.load(NshApp::Js(Bundle::source(
                        include_str!("../ui/bundle.js").to_owned(),
                    )));
                }
                view
            });
            boot_trace(&boot, "nsh view");
            // gpui-component widgets (the pages' inputs and menus) need a Root,
            // and the caption buttons sit above it: they belong to the window,
            // not to the pages (see `chrome`).
            let root = cx.new(|cx| gpui_component::Root::new(view, window, cx));
            let shell = cx.new(|_| Shell { root });
            boot_trace(&boot, "shell");
            shell
        })
        .expect("the Manager window opens");
        // Closing hides the window instead of ending the process, because the
        // pill is drawn here and dictation must keep working with the UI gone.
        let _ = opened.update(cx, |_, window, cx| {
            lifecycle::remember(window);
            lifecycle::guard_close(window, cx);
        });
        boot_trace(&boot, "guard close");
        if std::env::var("MUNDUS_MANAGER_OFFSCREEN").is_err() {
            cx.activate(true);
        }
        boot_trace(&boot, "activated");
        let seen = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let _ = opened.update(cx, move |_, window, _cx| {
            arm_frame_trace(window, boot, seen);
        });
    });
}
