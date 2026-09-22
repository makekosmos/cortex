use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ::gpui::{prelude::*, *};

use crate::app::ManagerApp;
use crate::theme::*;
pub struct FpsOverlay {
    manager: WeakEntity<ManagerApp>,
    armed: bool,
    last_frame: Instant,
    warmup_until: Instant,
    samples: VecDeque<f32>,
    span: f32,
    ema: f32,
    avg: f32,
    low1: f32,
    low01: f32,
    stats_at: Instant,
    repaint_at: Instant,
}

impl FpsOverlay {
    pub fn new(manager: WeakEntity<ManagerApp>) -> Self {
        let now = Instant::now();
        Self {
            manager,
            armed: false,
            last_frame: now,
            warmup_until: now + Duration::from_secs(4),
            samples: VecDeque::new(),
            span: 0.0,
            ema: 0.0,
            avg: 0.0,
            low1: 0.0,
            low01: 0.0,
            stats_at: now,
            repaint_at: now,
        }
    }

    fn arm(weak: WeakEntity<Self>, manager: WeakEntity<ManagerApp>, window: &mut Window) {
        window.on_next_frame(move |window, cx| {
            let enabled = manager.upgrade().is_some_and(|app| app.read(cx).dev_fps);
            let alive = weak
                .update(cx, |this, cx| {
                    if !enabled {
                        this.armed = false;
                        return false;
                    }
                    this.sample();
                    if this.repaint_at.elapsed() >= Duration::from_millis(30) {
                        this.repaint_at = Instant::now();
                        cx.notify();
                    }
                    true
                })
                .unwrap_or(false);
            if alive {
                Self::arm(weak, manager, window);
            }
        });
    }

    fn sample(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        if now < self.warmup_until || !(0.0..1.0).contains(&dt) {
            return;
        }
        let fps = 1.0 / dt;
        let alpha = 1.0 - (-dt / 0.1).exp();
        self.ema = if self.ema == 0.0 {
            fps
        } else {
            self.ema * (1.0 - alpha) + fps * alpha
        };
        self.samples.push_back(dt);
        self.span += dt;
        while self.span > 60.0 && self.samples.len() > 1 {
            self.span -= self.samples.pop_front().unwrap_or_default();
        }
        if self.stats_at.elapsed() >= Duration::from_secs(1) {
            let mut sorted: Vec<_> = self.samples.iter().copied().collect();
            sorted.sort_unstable_by(f32::total_cmp);
            let n = sorted.len();
            self.avg = n as f32 / self.span.max(1e-4);
            self.low1 = 1.0 / sorted[((n as f32 * 0.99) as usize).min(n - 1)].max(1e-4);
            self.low01 = 1.0 / sorted[((n as f32 * 0.999) as usize).min(n - 1)].max(1e-4);
            self.stats_at = now;
        }
    }
}

impl Render for FpsOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.armed {
            let now = Instant::now();
            self.armed = true;
            self.last_frame = now;
            self.warmup_until = now + Duration::from_secs(4);
            self.samples.clear();
            self.span = 0.0;
            self.ema = 0.0;
            Self::arm(cx.weak_entity(), self.manager.clone(), window);
        }

        let warming = Instant::now() < self.warmup_until;
        let mut elapsed = 0.0;
        let mut tail = Vec::new();
        for &dt in self.samples.iter().rev() {
            if elapsed > 5.0 {
                break;
            }
            elapsed += dt;
            tail.push(dt);
        }
        tail.reverse();
        let step = (tail.len() / 150).max(1);
        let samples: Vec<_> = tail.into_iter().step_by(step).collect();
        let accent = c(ACCENT());
        let warn = c(WARN());
        let guide = c(BORDER());

        div()
            .font_family("Inter")
            .text_color(c(FG()))
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(c(BORDER()))
            .bg(fade(BG(), 0.85))
            .flex()
            .items_center()
            .gap_2()
            .child(
                canvas(
                    move |_, _, _| samples,
                    move |bounds, samples, window, _| {
                        let w = f32::from(bounds.size.width);
                        let h = f32::from(bounds.size.height);
                        let y = |dt: f32| h - ((1.0 / dt.max(1e-4)) / 180.0).clamp(0.02, 1.0) * h;
                        let guide_y = y(1.0 / 165.0);
                        window.paint_quad(fill(
                            Bounds::from_corners(
                                point(bounds.origin.x, bounds.origin.y + px(guide_y)),
                                point(bounds.origin.x + px(w), bounds.origin.y + px(guide_y + 1.0)),
                            ),
                            guide,
                        ));
                        if samples.len() >= 2 {
                            let dx = w / (samples.len() - 1) as f32;
                            for i in 1..samples.len() {
                                let y0 = y(samples[i - 1]);
                                let y1 = y(samples[i]);
                                let x0 = bounds.origin.x + px((i - 1) as f32 * dx);
                                let x1 = (bounds.origin.x + px(i as f32 * dx)).max(x0 + px(1.5));
                                window.paint_quad(fill(
                                    Bounds::from_corners(
                                        point(x0, bounds.origin.y + px(y0.min(y1))),
                                        point(
                                            x1,
                                            bounds.origin.y + px(y0.max(y1).max(y0.min(y1) + 1.5)),
                                        ),
                                    ),
                                    if 1.0 / samples[i].max(1e-4) < 60.0 {
                                        warn
                                    } else {
                                        accent
                                    },
                                ));
                            }
                        }
                    },
                )
                .w(px(150.))
                .h(px(26.)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if warming {
                                "прогрев…".into()
                            } else {
                                format!("{:.0} fps", self.ema)
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(c(MUTED_FG()))
                            .child(if warming {
                                "замер начнётся после загрузки".into()
                            } else {
                                format!(
                                    "avg {:.0} · 1% {:.0} · 0.1% {:.0}",
                                    self.avg, self.low1, self.low01
                                )
                            }),
                    ),
            )
    }
}
