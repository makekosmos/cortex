// Phase 6: launcher window через eframe (egui) — Raycast/Spotlight-style.
//
// UX:
//   - Окно без рамки (no decorations), always-on-top, центрировано в верхней
//     трети монитора (как у Spotlight).
//   - Стартует 1×1 px offscreen → после первого frame ресайзится и
//     минимизируется. Это убирает «flash» при старте Kosmos.
//   - Ctrl+Shift+K → restore from minimize + focus search input.
//   - Esc / close button (X-винды нет, есть кастомная) → minimize обратно.
//   - Tray «Выход» → real close → eframe::run_native returns → Kosmos exits.
//
// FTS5 search идёт через `ArkHost::request("search_objects", {query})` напрямую.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use serde_json::json;

use crate::ark_host::ArkHost;
use crate::tray::LAUNCHER_REQUESTS;

const ACCENT_VIOLET: egui::Color32 = egui::Color32::from_rgb(155, 130, 255);
const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(24, 24, 28);
const BG_INPUT: egui::Color32 = egui::Color32::from_rgb(36, 36, 42);
const BG_HOVER: egui::Color32 = egui::Color32::from_rgb(44, 44, 52);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(232, 232, 240);
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(150, 150, 165);
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(130, 220, 150);
const ERROR: egui::Color32 = egui::Color32::from_rgb(255, 110, 110);

const WINDOW_W: f32 = 720.0;
const WINDOW_H_COMPACT: f32 = 76.0;   // только search bar
const WINDOW_H_EXPANDED: f32 = 460.0; // с результатами / quick-create
const OFFSCREEN_X: f32 = -10_000.0;
const OFFSCREEN_Y: f32 = -10_000.0;

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    snippet: String,
    entity_id: String,
}

pub fn run_launcher(ark: Arc<ArkHost>, rt: tokio::runtime::Handle) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kepler Kosmos")
            // Стартует 1×1 px offscreen — пользователь вспышку не видит.
            .with_inner_size([1.0, 1.0])
            .with_position([-10_000.0, -10_000.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_visible(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_taskbar(false),
        ..Default::default()
    };

    let app = LauncherApp::new(ark, rt);

    if let Err(e) = eframe::run_native(
        "Kepler Kosmos",
        options,
        Box::new(|cc| {
            apply_kosmos_style(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    ) {
        eprintln!("[kosmos.launcher] eframe error: {e}");
    }
}

fn apply_kosmos_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_fill = egui::Color32::TRANSPARENT;
    visuals.panel_fill = egui::Color32::TRANSPARENT;
    visuals.selection.bg_fill = ACCENT_VIOLET.linear_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_VIOLET);
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.inactive.bg_fill = BG_INPUT;
    visuals.widgets.hovered.bg_fill = BG_HOVER;
    visuals.widgets.active.bg_fill = ACCENT_VIOLET.linear_multiply(0.4);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    ctx.set_style(style);
}

struct LauncherApp {
    ark: Arc<ArkHost>,
    rt: tokio::runtime::Handle,
    last_seen_requests: u64,
    visible: bool,
    initial_setup_done: bool,
    query: String,
    last_searched_query: String,
    results: Vec<SearchResult>,
    search_error: Option<String>,
    flash_message: Option<(Instant, String)>,
    focus_input_next_frame: bool,
}

impl LauncherApp {
    fn new(ark: Arc<ArkHost>, rt: tokio::runtime::Handle) -> Self {
        Self {
            ark,
            rt,
            last_seen_requests: LAUNCHER_REQUESTS.load(Ordering::SeqCst),
            visible: false,
            initial_setup_done: false,
            query: String::new(),
            last_searched_query: String::new(),
            results: Vec::new(),
            search_error: None,
            flash_message: None,
            focus_input_next_frame: false,
        }
    }

    fn create_object(&mut self, type_id: &str, kind_label: &str) {
        let title = self.query.trim().to_string();
        if title.is_empty() {
            return;
        }
        let now = chrono::Utc::now().to_rfc3339();
        let object = json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "typeId": type_id,
            "title": title,
            "contentJson": {},
            "propsJson": {},
            "createdAt": now,
            "updatedAt": now,
            "deletedAt": null,
        });
        let ark = self.ark.clone();
        let result = self.rt.block_on(async move {
            ark.request(
                "upsert_object",
                json!({ "object": object, "device_id": "kosmos-launcher" }),
            )
            .await
        });
        match result {
            Ok(resp) if resp.ok => {
                self.flash_message = Some((
                    Instant::now(),
                    format!("✓ Создан{}: {}", kind_label, title),
                ));
                self.query.clear();
                self.results.clear();
                self.last_searched_query.clear();
            }
            Ok(resp) => {
                self.search_error =
                    Some(resp.error.unwrap_or_else(|| "(no error)".to_string()));
            }
            Err(e) => {
                self.search_error = Some(format!("create failed: {e}"));
            }
        }
    }

    fn run_search(&mut self) {
        let trimmed = self.query.trim();
        if trimmed.is_empty() {
            self.results.clear();
            self.search_error = None;
            self.last_searched_query.clear();
            return;
        }
        if trimmed == self.last_searched_query {
            return;
        }
        self.last_searched_query = trimmed.to_string();
        let ark = self.ark.clone();
        let q = trimmed.to_string();
        let result = self.rt.block_on(async move {
            ark.request("search_objects", json!({ "query": q })).await
        });
        match result {
            Ok(resp) if resp.ok => {
                let arr = resp.data.as_array().cloned().unwrap_or_default();
                self.results = arr
                    .into_iter()
                    .take(20)
                    .map(|v| SearchResult {
                        title: v.get("text").and_then(|t| t.as_str()).unwrap_or("(без названия)").to_string(),
                        snippet: v.get("file").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                        entity_id: v.get("entryId").or_else(|| v.get("entry_id")).and_then(|t| t.as_str()).unwrap_or("").to_string(),
                    })
                    .collect();
                self.search_error = None;
            }
            Ok(resp) => {
                self.search_error = Some(resp.error.unwrap_or_else(|| "(no error)".to_string()));
                self.results.clear();
            }
            Err(e) => {
                self.search_error = Some(format!("search failed: {e}"));
                self.results.clear();
            }
        }
    }

    /// Показать окно: переместить в центр + установить нужную высоту. Instant
    /// (без minimize animation — мы переключаемся через offscreen/onscreen position).
    fn show_window(&self, ctx: &egui::Context, expanded: bool) {
        let height = if expanded { WINDOW_H_EXPANDED } else { WINDOW_H_COMPACT };
        let monitor = ctx.input(|i| i.viewport().monitor_size)
            .unwrap_or(egui::vec2(1920.0, 1080.0));
        let x = ((monitor.x - WINDOW_W) / 2.0).max(0.0);
        // Top edge остаётся фиксированной (search bar на одной и той же Y), а
        // expand растёт вниз. Так glance не съезжает при добавлении результатов.
        let y = ((monitor.y - WINDOW_H_EXPANDED) / 2.0).max(0.0);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(WINDOW_W, height)));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    /// Скрыть окно: моментально уехать offscreen + 1x1. Никакой Windows-minimize animation.
    fn hide_window(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
            egui::pos2(OFFSCREEN_X, OFFSCREEN_Y),
        ));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1.0, 1.0)));
    }
}

impl eframe::App for LauncherApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Transparent window → full custom Frame внутри.
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // === Initial setup (first frame): сразу уехать offscreen, никакого flash ===
        if !self.initial_setup_done {
            self.hide_window(ctx);
            self.initial_setup_done = true;
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }

        // === Shutdown propagation ===
        if crate::tray::shutdown_requested() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // === Intercept window close button → hide вместо close ===
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.visible = false;
            self.hide_window(ctx);
            return;
        }

        // === Poll hotkey counter — show + focus ===
        let current = LAUNCHER_REQUESTS.load(Ordering::SeqCst);
        if current > self.last_seen_requests {
            self.last_seen_requests = current;
            self.visible = true;
            self.query.clear();
            self.results.clear();
            self.search_error = None;
            self.last_searched_query.clear();
            self.focus_input_next_frame = true;
            // При открытии — compact mode (только search bar).
            self.show_window(ctx, false);
        }

        if !self.visible {
            ctx.request_repaint_after(Duration::from_millis(200));
            return;
        }

        // Esc → hide
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.visible = false;
            self.hide_window(ctx);
            return;
        }

        // Dynamic resize: если query непустой OR есть results — expanded; иначе compact.
        let has_content = !self.query.trim().is_empty()
            || !self.results.is_empty()
            || self.search_error.is_some()
            || self.flash_message.is_some();
        if has_content {
            // Только меняем height, position остаётся той же (top edge fixed).
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                egui::vec2(WINDOW_W, WINDOW_H_EXPANDED),
            ));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                egui::vec2(WINDOW_W, WINDOW_H_COMPACT),
            ));
        }

        // === UI ===
        let panel_frame = egui::Frame::new()
            .fill(BG_PANEL)
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(20))
            .shadow(egui::epaint::Shadow {
                offset: [0, 4],
                blur: 24,
                spread: 0,
                color: egui::Color32::from_black_alpha(140),
            })
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 70)));

        egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
            // --- Search input row ---
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⌕").size(28.0).color(ACCENT_VIOLET));
                let input_frame = egui::Frame::new()
                    .fill(BG_INPUT)
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(14, 10));
                input_frame.show(ui, |ui| {
                    ui.style_mut().visuals.extreme_bg_color = BG_INPUT;
                    let text_response = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text(
                                egui::RichText::new("Поиск или быстрое создание...")
                                    .color(TEXT_SECONDARY)
                                    .size(20.0),
                            )
                            .desired_width(WINDOW_W - 130.0)
                            .frame(false)
                            .font(egui::FontId::proportional(22.0))
                            .text_color(TEXT_PRIMARY),
                    );
                    if self.focus_input_next_frame {
                        text_response.request_focus();
                        self.focus_input_next_frame = false;
                    }
                    if text_response.changed() {
                        self.run_search();
                    }
                });
            });

            ui.add_space(14.0);

            // --- Results / placeholder / quick-create ---
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if let Some(err) = &self.search_error {
                    ui.colored_label(ERROR, format!("⚠ {err}"));
                    ui.add_space(8.0);
                }

                let query_trimmed = self.query.trim().to_string();

                // Когда пусто — НЕ показываем хинт-текст: в compact mode
                // окна одна полоска поиска и всё. Минимализм как у Raycast.
                if self.results.is_empty() && self.search_error.is_none() && !query_trimmed.is_empty() {
                    ui.label(egui::RichText::new("Ничего не найдено в ARK")
                        .color(TEXT_SECONDARY).size(15.0));
                }

                for (_idx, r) in self.results.iter().enumerate() {
                    let item_frame = egui::Frame::new()
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 10));
                    let response = item_frame.show(ui, |ui| {
                        ui.label(egui::RichText::new(&r.title).color(TEXT_PRIMARY).size(15.0).strong());
                        if !r.snippet.is_empty() {
                            ui.label(egui::RichText::new(&r.snippet).color(TEXT_SECONDARY).size(12.0));
                        }
                    });
                    if response.response.hovered() {
                        ui.painter().rect_filled(response.response.rect, egui::CornerRadius::same(8), BG_HOVER);
                    }
                    ui.add_space(2.0);
                }

                // Quick-create buttons
                if !query_trimmed.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Быстрое создание")
                        .color(TEXT_SECONDARY).size(12.0));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.add(
                            egui::Button::new(egui::RichText::new(format!("✚ Задача: «{}»", query_trimmed))
                                .color(TEXT_PRIMARY).size(13.0))
                                .fill(BG_INPUT)
                                .corner_radius(egui::CornerRadius::same(8))
                        ).clicked() {
                            self.create_object("task_obj", "а задача");
                        }
                        if ui.add(
                            egui::Button::new(egui::RichText::new(format!("✚ Заметка: «{}»", query_trimmed))
                                .color(TEXT_PRIMARY).size(13.0))
                                .fill(BG_INPUT)
                                .corner_radius(egui::CornerRadius::same(8))
                        ).clicked() {
                            self.create_object("note_obj", "а заметка");
                        }
                    });
                }

                // Flash message
                if let Some((created_at, msg)) = self.flash_message.clone() {
                    if created_at.elapsed() < Duration::from_secs(3) {
                        ui.add_space(8.0);
                        ui.colored_label(SUCCESS, &msg);
                    } else {
                        self.flash_message = None;
                    }
                }
            });
        });

        ctx.request_repaint_after(Duration::from_millis(150));
    }
}
