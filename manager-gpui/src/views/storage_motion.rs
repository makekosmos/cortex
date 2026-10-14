use crate::app::ManagerApp;
use gpui::{Context, Pixels, Point, ScrollAnchor, ScrollHandle, Window};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    time::Instant,
};

#[derive(Default)]
pub struct StorageUi {
    pub scroll: ScrollHandle,
    pub open: HashSet<String>,
    pub hovered: Option<String>,
    anchors: RefCell<HashMap<String, ScrollAnchor>>,
    motion: Option<ScrollMotion>,
    generation: u64,
}

struct ScrollMotion {
    started: Instant,
    from: Point<Pixels>,
    to: Point<Pixels>,
}

impl StorageUi {
    pub fn anchor(&self, id: &str) -> ScrollAnchor {
        self.anchors
            .borrow_mut()
            .entry(id.to_owned())
            .or_insert_with(|| ScrollAnchor::for_handle(self.scroll.clone()))
            .clone()
    }

    pub fn advance(&mut self, window: &Window) {
        let Some(motion) = &self.motion else { return };
        let t = (motion.started.elapsed().as_secs_f32() / 0.75).min(1.);
        let eased = t * t * t * (t * (6. * t - 15.) + 10.);
        self.scroll
            .set_offset(motion.from + (motion.to - motion.from) * eased);
        if t < 1. {
            window.request_animation_frame();
        } else {
            self.motion = None;
        }
    }

    pub fn reveal(
        app: &mut ManagerApp,
        id: String,
        window: &mut Window,
        cx: &mut Context<ManagerApp>,
    ) {
        app.storage_ui.open.insert(id.clone());
        app.storage_ui.generation += 1;
        let generation = app.storage_ui.generation;
        let from = app.storage_ui.scroll.offset();
        app.storage_ui.anchor(&id).scroll_to(window, cx);
        // ScrollAnchor resolves its destination on the next layout. Capture that
        // destination after its callback, then interpolate instead of jumping.
        cx.on_next_frame(window, move |app, _, cx| {
            if generation != app.storage_ui.generation {
                return;
            }
            let to = app.storage_ui.scroll.offset();
            if cx.reduce_motion() {
                app.storage_ui.motion = None;
            } else {
                app.storage_ui.scroll.set_offset(from);
                app.storage_ui.motion = Some(ScrollMotion {
                    started: Instant::now(),
                    from,
                    to,
                });
            }
            cx.notify();
        });
        cx.notify();
    }
}
