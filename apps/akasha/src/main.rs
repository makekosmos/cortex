mod epub;
mod state;

use crate::epub::{read_epub, BlockKind, Book, InlineSpan, ReaderBlock};
use crate::state::ReaderState;
use gpui::{
    div, prelude::*, px, rems, rgb, size, AnyElement, App, Application, AsyncApp, Bounds, Context,
    FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollStrategy, SharedString, Size, Task, Timer, WeakEntity, Window, WindowBounds,
    WindowOptions,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex, input,
    scroll::{ScrollableElement, Scrollbar},
    text::TextView,
    text::TextViewStyle,
    v_flex, v_virtual_list, Sizable, VirtualListScrollHandle,
};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

const FONT_FAMILIES: [&str; 4] = ["Georgia", "Palatino", "Inter", "Geist"];
const EPUB_FONT_SCALE_H1: f32 = 2.0;
const EPUB_FONT_SCALE_H2: f32 = 1.5;
const EPUB_FONT_SCALE_H3: f32 = 1.4;
const EPUB_FONT_SCALE_H4: f32 = 1.2;
const EPUB_FONT_SCALE_H5: f32 = 1.1;
const EPUB_FONT_SCALE_H6: f32 = 1.0;
const EPUB_FONT_SCALE_BLOCKQUOTE: f32 = 1.1;
const EPUB_LINE_HEIGHT_BODY: f32 = 1.5;
const EPUB_LINE_HEIGHT_HEADING: f32 = 1.2;
const EPUB_LINE_HEIGHT_BLOCKQUOTE: f32 = 1.4;
const EPUB_HEADING_MARGIN_TOP_H1: f32 = 3.0;
const EPUB_HEADING_MARGIN_BOTTOM_H1: f32 = 1.0;
const EPUB_HEADING_MARGIN_TOP_H2: f32 = 2.0;
const EPUB_HEADING_MARGIN_BOTTOM_H2: f32 = 0.6;
const EPUB_HEADING_MARGIN_TOP_H3: f32 = 1.5;
const EPUB_HEADING_MARGIN_BOTTOM_H3: f32 = 0.4;
const EPUB_HEADING_MARGIN_TOP_H4: f32 = 1.2;
const EPUB_HEADING_MARGIN_BOTTOM_H4: f32 = 0.3;
const EPUB_HEADING_MARGIN_TOP_H5_H6: f32 = 1.0;
const EPUB_HEADING_MARGIN_BOTTOM_H5_H6: f32 = 0.25;
const EPUB_BLOCKQUOTE_MARGIN_V: f32 = 1.0;
const EPUB_CONTENT_MAX_WIDTH_CH: f32 = 70.0;
const EPUB_PARAGRAPH_BLOCK_MARGIN_BOTTOM: f32 = 0.75;
const EPUB_LIST_MARGIN_BOTTOM: f32 = 0.35;
const READER_MAX_WIDTH: f32 = 780.0;
const READER_HORIZONTAL_PADDING: f32 = 64.0;

struct Akasha {
    user_data_dir: PathBuf,
    state: ReaderState,
    book: Option<Book>,
    reader_blocks: Vec<ReaderBlock>,
    reader_item_sizes: Rc<Vec<Size<Pixels>>>,
    reader_layout_width: f32,
    toc_open: bool,
    settings_open: bool,
    reader_scroll: VirtualListScrollHandle,
    auto_scroll_origin_y: Option<Pixels>,
    auto_scroll_step: Pixels,
    auto_scroll_task: Option<Task<()>>,
    selection_drag_start: Option<Point<Pixels>>,
    selection_tip_at: Option<Point<Pixels>>,
    selection_tip_task: Option<Task<()>>,
    dev_fps_enabled: bool,
    fps_frames: u32,
    fps_value: f32,
    fps_last_sample: Instant,
    fps_task: Option<Task<()>>,
    status: SharedString,
}

impl Akasha {
    fn new(user_data_dir: PathBuf, dev_fps_enabled: bool) -> Self {
        let state = ReaderState::load(&user_data_dir);
        let mut app = Self {
            user_data_dir,
            state,
            book: None,
            reader_blocks: Vec::new(),
            reader_item_sizes: Rc::new(Vec::new()),
            reader_layout_width: READER_MAX_WIDTH,
            toc_open: false,
            settings_open: false,
            reader_scroll: VirtualListScrollHandle::new(),
            auto_scroll_origin_y: None,
            auto_scroll_step: px(0.0),
            auto_scroll_task: None,
            selection_drag_start: None,
            selection_tip_at: None,
            selection_tip_task: None,
            dev_fps_enabled,
            fps_frames: 0,
            fps_value: 0.0,
            fps_last_sample: Instant::now(),
            fps_task: None,
            status: "Open an EPUB to begin.".into(),
        };
        if let Some(path) = app.state.last_path.clone() {
            app.open_book(&path);
        }
        app
    }

    fn open_book(&mut self, path: &Path) {
        match read_epub(path) {
            Ok(book) => {
                self.state.last_path = Some(path.to_path_buf());
                self.state.chapter_index = self.state.chapter_index.min(book.chapters.len() - 1);
                self.rebuild_reader_cache(&book);
                let _ = self.state.save(&self.user_data_dir);
                self.status = format!("Opened {}", path.display()).into();
                self.book = Some(book);
            }
            Err(err) => {
                self.status = format!("Could not open EPUB: {err}").into();
            }
        }
    }

    fn save_state(&self) {
        let _ = self.state.save(&self.user_data_dir);
    }

    fn rebuild_reader_cache(&mut self, book: &Book) {
        self.reader_blocks = book
            .chapters
            .iter()
            .flat_map(|chapter| chapter.blocks.iter().cloned())
            .collect();
        self.rebuild_reader_sizes();
    }

    fn rebuild_reader_sizes(&mut self) {
        self.reader_item_sizes = Rc::new(
            self.reader_blocks
                .iter()
                .map(|block| {
                    estimated_block_size(block, self.state.font_size, self.reader_layout_width)
                })
                .collect(),
        );
    }

    fn update_reader_layout_width(&mut self, viewport_width: Pixels) {
        let width = ((viewport_width - px(READER_HORIZONTAL_PADDING)) / px(1.0))
            .clamp(280.0, READER_MAX_WIDTH);
        if (self.reader_layout_width - width).abs() > 1.0 {
            self.reader_layout_width = width;
            self.rebuild_reader_sizes();
        }
    }

    fn start_auto_scroll(&mut self, origin_y: Pixels, cx: &mut Context<Self>) {
        self.auto_scroll_origin_y = Some(origin_y);
        self.auto_scroll_step = px(0.0);
        self.status = "Autoscroll: move mouse above or below the marker.".into();

        self.auto_scroll_task = Some(cx.spawn(auto_scroll_loop));
    }

    fn stop_auto_scroll(&mut self) {
        self.auto_scroll_origin_y = None;
        self.auto_scroll_step = px(0.0);
        self.auto_scroll_task = None;
    }

    fn update_auto_scroll_speed(&mut self, y: Pixels) {
        let Some(origin_y) = self.auto_scroll_origin_y else {
            return;
        };
        let delta = y - origin_y;
        self.auto_scroll_step = if delta.abs() < px(8.0) {
            px(0.0)
        } else {
            delta * 0.10
        };
    }

    fn maybe_show_selection_tip(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(start) = self.selection_drag_start.take() else {
            return;
        };
        let dragged =
            (position.x - start.x).abs() > px(4.0) || (position.y - start.y).abs() > px(4.0);
        if !dragged {
            return;
        }

        self.selection_tip_at = Some(position);
        self.selection_tip_task = Some(cx.spawn(clear_selection_tip_later));
    }

    fn tick_fps_counter(&mut self, cx: &mut Context<Self>) {
        if !self.dev_fps_enabled {
            return;
        }

        self.fps_frames = self.fps_frames.saturating_add(1);
        if self.fps_task.is_none() {
            self.fps_task = Some(cx.spawn(fps_meter_loop));
        }
    }
}

impl Render for Akasha {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tick_fps_counter(cx);
        self.update_reader_layout_width(window.viewport_size().width);

        let title = self
            .book
            .as_ref()
            .map(|book| book.title.clone())
            .unwrap_or_else(|| "Akasha".to_string());
        let font_size = self.state.font_size;
        let font_family = self.state.font_family.clone();
        let reader_layout_width = self.reader_layout_width;
        let reader_item_sizes = self.reader_item_sizes.clone();
        let reader_scroll = self.reader_scroll.clone();
        let reader_scrollbar = self.reader_scroll.clone();
        let settings_items: Vec<AnyElement> = FONT_FAMILIES
            .iter()
            .enumerate()
            .map(|(font_index, family)| {
                let selected = *family == self.state.font_family;
                Button::new(("font", font_index))
                    .label(*family)
                    .when(selected, |this| this.primary())
                    .on_click(cx.listener({
                        let family = (*family).to_string();
                        move |this, _, _window, _cx| {
                            this.state.font_family = family.clone();
                            this.save_state();
                        }
                    }))
                    .into_any_element()
            })
            .collect();
        let chapter_items: Vec<AnyElement> = self
            .book
            .as_ref()
            .map(|book| {
                book.chapters
                    .iter()
                    .enumerate()
                    .map(|(index, chapter)| {
                        let selected = index == self.state.chapter_index;
                        div()
                            .rounded_md()
                            .px_3()
                            .py_2()
                            .bg(if selected {
                                rgb(0x2b261e)
                            } else {
                                rgb(0x151411)
                            })
                            .text_color(if selected {
                                rgb(0xffd08a)
                            } else {
                                rgb(0xd8d0c3)
                            })
                            .child(chapter.title.clone())
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |this, _, _window, _cx| {
                                    this.state.chapter_index = index;
                                    if let Some(book) = &this.book {
                                        this.reader_scroll.scroll_to_item(
                                            book.chapter_start_block(index),
                                            ScrollStrategy::Top,
                                        );
                                    }
                                    this.toc_open = false;
                                    this.save_state();
                                }),
                            )
                            .into_any_element()
                    })
                    .collect()
            })
            .unwrap_or_default();

        v_flex()
            .size_full()
            .bg(rgb(0x10100f))
            .text_color(rgb(0xf1eee7))
            .child(
                h_flex()
                    .h(px(54.0))
                    .px_4()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(0x2b2924))
                    .child(
                        Button::new("open")
                            .label("Открыть книгу")
                            .on_click(cx.listener(|this, _, _window, _cx| {
                                if let Some(path) =
                                    rfd::FileDialog::new().add_filter("EPUB", &["epub"]).pick_file()
                                {
                                    this.open_book(&path);
                                }
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(18.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        Button::new("toc")
                            .label("☰")
                            .on_click(cx.listener(|this, _, _window, _cx| {
                                this.toc_open = !this.toc_open;
                                this.settings_open = false;
                            })),
                    )
                    .child(
                        Button::new("settings")
                            .label("Aa")
                            .on_click(cx.listener(|this, _, _window, _cx| {
                                this.settings_open = !this.settings_open;
                                this.toc_open = false;
                            })),
                    )
                    .child(
                        Button::new("font-down")
                            .label("A-")
                            .on_click(cx.listener(|this, _, _window, _cx| {
                                this.state.font_size = (this.state.font_size - 1.0).max(13.0);
                                this.rebuild_reader_sizes();
                                this.save_state();
                            })),
                    )
                    .child(
                        Button::new("font-up")
                            .label("A+")
                            .on_click(cx.listener(|this, _, _window, _cx| {
                                this.state.font_size = (this.state.font_size + 1.0).min(32.0);
                                this.rebuild_reader_sizes();
                                this.save_state();
                            })),
                    ),
            )
            .when(self.toc_open, |this| {
                this.child(
                    v_flex()
                        .max_h(px(260.0))
                        .overflow_y_scrollbar()
                        .mx_4()
                        .my_3()
                        .p_3()
                        .gap_1()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0x3a352e))
                        .bg(rgb(0x151411))
                        .children(chapter_items),
                )
            })
            .when(self.settings_open, |this| {
                this.child(
                    h_flex()
                        .mx_4()
                        .my_3()
                        .p_3()
                        .gap_2()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0x3a352e))
                        .bg(rgb(0x151411))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgb(0xc9bfae))
                                .child("Font"),
                        )
                        .children(settings_items),
                )
            })
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .relative()
                    .font_family(font_family.clone())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _window, _cx| {
                            if this.auto_scroll_origin_y.is_some() {
                                this.stop_auto_scroll();
                            }
                            this.selection_tip_at = None;
                            this.selection_drag_start = Some(event.position);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseUpEvent, _window, cx| {
                            this.maybe_show_selection_tip(event.position, cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                            if this.auto_scroll_origin_y.is_some() {
                                this.stop_auto_scroll();
                            } else {
                                this.start_auto_scroll(event.position.y, cx);
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                        if this.auto_scroll_origin_y.is_some() {
                            this.update_auto_scroll_speed(event.position.y);
                            cx.notify();
                        };
                    }))
                    .when(self.book.is_some(), |this| {
                        this.child(
                            div()
                                .relative()
                                .size_full()
                                .child(
                                    v_virtual_list(cx.entity(), "reader-list", reader_item_sizes, {
                                        move |this, range, _window, _cx| {
                                            let start = range.start;
                                            this.reader_blocks[range]
                                                .iter()
                                                .cloned()
                                                .enumerate()
                                                .map(|(offset, block)| {
                                                    render_reader_block(
                                                        block,
                                                        start + offset,
                                                        font_size,
                                                        &font_family,
                                                        reader_layout_width,
                                                        _window,
                                                        _cx,
                                                    )
                                                })
                                                .collect::<Vec<_>>()
                                        }
                                    })
                                    .track_scroll(&reader_scroll)
                                    .p_8(),
                                )
                                .child(Scrollbar::vertical(&reader_scrollbar)),
                        )
                    })
                    .when_some(self.selection_tip_at, |this, position| {
                        this.child(
                            h_flex()
                                .absolute()
                                .left(position.x)
                                .top(position.y + px(10.0))
                                .gap_1()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(0x4c4438))
                                .bg(rgb(0x1d1a16))
                                .text_color(rgb(0xf1eee7))
                                .text_size(px(12.0))
                                .child("Selection")
                                .child(
                                    Button::new("copy-selection")
                                        .label("Copy")
                                        .small()
                                        .on_click(cx.listener(|this, _, _window, cx| {
                                            cx.dispatch_action(&input::Copy);
                                            this.selection_tip_at = None;
                                            this.status = "Copied selection.".into();
                                        })),
                                ),
                        )
                    })
                    .when(self.dev_fps_enabled, |this| {
                        this.child(
                            h_flex()
                                .absolute()
                                .top(px(18.0))
                                .right(px(24.0))
                                .gap_1()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(0x4c4438))
                                .bg(rgb(0x1d1a16))
                                .text_color(rgb(0xffd08a))
                                .text_size(px(12.0))
                                .child(format!("FPS {:.0}", self.fps_value)),
                        )
                    })
                    .when(self.auto_scroll_origin_y.is_some(), |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(18.0))
                                .left(px(24.0))
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(0xffd08a))
                                .bg(rgb(0x2b261e))
                                .text_color(rgb(0xffd08a))
                                .text_size(px(12.0))
                                .child("● autoscroll"),
                        )
                    })
                    .when(self.book.is_none(), |this| {
                        this.child(
                            div()
                                .p_8()
                                .max_w(px(560.0))
                                .text_size(px(18.0))
                                .line_height(px(28.0))
                                .text_color(rgb(0xc9bfae))
                                .child("Akasha is a native GPUI EPUB reader for Kosmos. Use Open EPUB to load a book, or launch with --open <path>."),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(32.0))
                    .px_4()
                    .border_t_1()
                    .border_color(rgb(0x2b2924))
                    .text_size(px(12.0))
                    .text_color(rgb(0x9f9688))
                    .child(self.status.clone()),
            )
    }
}

async fn auto_scroll_loop(this: WeakEntity<Akasha>, cx: &mut AsyncApp) {
    loop {
        Timer::after(Duration::from_millis(16)).await;
        let keep_scrolling = this
            .update(cx, |this, cx| {
                if this.auto_scroll_origin_y.is_none() {
                    return false;
                }
                if this.auto_scroll_step.abs() >= px(0.5) {
                    let mut offset = this.reader_scroll.offset();
                    offset.y += this.auto_scroll_step;
                    this.reader_scroll.set_offset(offset);
                    cx.notify();
                }
                true
            })
            .unwrap_or(false);
        if !keep_scrolling {
            break;
        }
    }
}

async fn clear_selection_tip_later(this: WeakEntity<Akasha>, cx: &mut AsyncApp) {
    Timer::after(Duration::from_secs(4)).await;
    _ = this.update(cx, |this, cx| {
        this.selection_tip_at = None;
        this.selection_tip_task = None;
        cx.notify();
    });
}

async fn fps_meter_loop(this: WeakEntity<Akasha>, cx: &mut AsyncApp) {
    loop {
        Timer::after(Duration::from_millis(500)).await;
        let keep_measuring = this
            .update(cx, |this, cx| {
                if !this.dev_fps_enabled {
                    this.fps_task = None;
                    return false;
                }

                let now = Instant::now();
                let elapsed = now.duration_since(this.fps_last_sample).as_secs_f32();
                if elapsed > 0.0 {
                    this.fps_value = this.fps_frames as f32 / elapsed;
                }
                this.fps_frames = 0;
                this.fps_last_sample = now;
                cx.notify();
                true
            })
            .unwrap_or(false);
        if !keep_measuring {
            break;
        }
    }
}

fn estimated_block_size(block: &ReaderBlock, font_size: f32, layout_width: f32) -> Size<Pixels> {
    let text_len = block.plain_text().chars().count().max(1) as f32;
    let average_char_width = font_size * 0.52;
    let chars_per_line = (layout_width / average_char_width).clamp(18.0, EPUB_CONTENT_MAX_WIDTH_CH);
    let lines = (text_len / chars_per_line).ceil().max(1.0);
    let height = match block.kind {
        BlockKind::Heading(level) => {
            let heading = heading_size(font_size, level);
            heading * EPUB_LINE_HEIGHT_HEADING
                + heading_margin_top(font_size, level)
                + heading_margin_bottom(font_size, level)
        }
        BlockKind::Paragraph => {
            lines * body_line_height(font_size) + paragraph_margin_bottom(font_size)
        }
        BlockKind::ListItem => lines * body_line_height(font_size) + list_margin_bottom(font_size),
        BlockKind::Blockquote => {
            lines * blockquote_line_height(font_size) + font_size * EPUB_BLOCKQUOTE_MARGIN_V * 2.0
        }
    };
    size(px(layout_width), px(height))
}

fn render_reader_block(
    block: ReaderBlock,
    index: usize,
    font_size: f32,
    font_family: &str,
    layout_width: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match block.kind {
        BlockKind::Heading(level) => div()
            .max_w(px(layout_width))
            .mt(px(heading_margin_top(font_size, level)))
            .mb(px(heading_margin_bottom(font_size, level)))
            .text_size(px(heading_size(font_size, level)))
            .line_height(px(heading_size(font_size, level) * EPUB_LINE_HEIGHT_HEADING))
            .font_weight(heading_weight(level))
            .text_color(if level == 1 {
                rgb(0xffd08a)
            } else {
                rgb(0xf2dfbd)
            })
            .child(selectable_markdown(
                index,
                block_to_markdown(&block),
                font_size,
                font_family,
                window,
                cx,
            ))
            .into_any_element(),
        BlockKind::Paragraph => div()
            .max_w(px(layout_width))
            .mb(px(paragraph_margin_bottom(font_size)))
            .text_size(px(font_size))
            .line_height(px(body_line_height(font_size)))
            .font_weight(FontWeight::NORMAL)
            .text_color(rgb(0xf1eee7))
            .child(selectable_markdown(
                index,
                block_to_markdown(&block),
                font_size,
                font_family,
                window,
                cx,
            ))
            .into_any_element(),
        BlockKind::ListItem => h_flex()
            .max_w(px(layout_width))
            .mb(px(list_margin_bottom(font_size)))
            .items_start()
            .gap_3()
            .child(
                div()
                    .pt(px(font_size * 0.5))
                    .text_size(px(font_size))
                    .text_color(rgb(0xffd08a))
                    .child("•"),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(font_size))
                    .line_height(px(body_line_height(font_size)))
                    .child(selectable_markdown(
                        index,
                        block_to_markdown(&block),
                        font_size,
                        font_family,
                        window,
                        cx,
                    )),
            )
            .into_any_element(),
        BlockKind::Blockquote => div()
            .max_w(px(layout_width))
            .pl_4()
            .my(px(font_size * EPUB_BLOCKQUOTE_MARGIN_V))
            .border_l_3()
            .border_color(rgb(0xffd08a))
            .text_size(px(font_size * EPUB_FONT_SCALE_BLOCKQUOTE))
            .line_height(px(blockquote_line_height(font_size)))
            .italic()
            .text_color(rgb(0xd8d0c3))
            .child(selectable_markdown(
                index,
                block_to_markdown(&block),
                font_size,
                font_family,
                window,
                cx,
            ))
            .into_any_element(),
    }
}

fn selectable_markdown(
    index: usize,
    markdown: String,
    font_size: f32,
    font_family: &str,
    window: &mut Window,
    cx: &mut App,
) -> TextView {
    TextView::markdown(("reader-block", index), markdown, window, cx)
        .font_family(font_family.to_string())
        .style(
            TextViewStyle::default()
                .paragraph_gap(rems(0.))
                .heading_font_size(move |level, _base| px(heading_size(font_size, level))),
        )
        .selectable(true)
}

fn block_to_markdown(block: &ReaderBlock) -> String {
    let text = spans_to_markdown(&block.spans);
    match block.kind {
        BlockKind::Heading(level) => format!("{} {text}", "#".repeat(level as usize)),
        BlockKind::Paragraph | BlockKind::ListItem | BlockKind::Blockquote => text,
    }
}

fn spans_to_markdown(spans: &[InlineSpan]) -> String {
    spans
        .iter()
        .map(|span| {
            let text = escape_markdown(&span.text);
            match (span.strong, span.emphasis) {
                (true, true) => format!("***{text}***"),
                (true, false) => format!("**{text}**"),
                (false, true) => format!("_{text}_"),
                (false, false) => text,
            }
        })
        .collect()
}

fn escape_markdown(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' | '*' | '_' | '`' | '[' | ']' | '#' => ['\\', ch].into_iter().collect::<Vec<_>>(),
            _ => [ch].into_iter().collect(),
        })
        .collect()
}

fn heading_size(font_size: f32, level: u8) -> f32 {
    match level {
        1 => font_size * EPUB_FONT_SCALE_H1,
        2 => font_size * EPUB_FONT_SCALE_H2,
        3 => font_size * EPUB_FONT_SCALE_H3,
        4 => font_size * EPUB_FONT_SCALE_H4,
        5 => font_size * EPUB_FONT_SCALE_H5,
        _ => font_size * EPUB_FONT_SCALE_H6,
    }
}

fn body_line_height(font_size: f32) -> f32 {
    font_size * EPUB_LINE_HEIGHT_BODY
}

fn blockquote_line_height(font_size: f32) -> f32 {
    font_size * EPUB_FONT_SCALE_BLOCKQUOTE * EPUB_LINE_HEIGHT_BLOCKQUOTE
}

fn paragraph_margin_bottom(font_size: f32) -> f32 {
    font_size * EPUB_PARAGRAPH_BLOCK_MARGIN_BOTTOM
}

fn list_margin_bottom(font_size: f32) -> f32 {
    font_size * EPUB_LIST_MARGIN_BOTTOM
}

fn heading_margin_top(font_size: f32, level: u8) -> f32 {
    font_size
        * match level {
            1 => EPUB_HEADING_MARGIN_TOP_H1,
            2 => EPUB_HEADING_MARGIN_TOP_H2,
            3 => EPUB_HEADING_MARGIN_TOP_H3,
            4 => EPUB_HEADING_MARGIN_TOP_H4,
            _ => EPUB_HEADING_MARGIN_TOP_H5_H6,
        }
}

fn heading_margin_bottom(font_size: f32, level: u8) -> f32 {
    font_size
        * match level {
            1 => EPUB_HEADING_MARGIN_BOTTOM_H1,
            2 => EPUB_HEADING_MARGIN_BOTTOM_H2,
            3 => EPUB_HEADING_MARGIN_BOTTOM_H3,
            4 => EPUB_HEADING_MARGIN_BOTTOM_H4,
            _ => EPUB_HEADING_MARGIN_BOTTOM_H5_H6,
        }
}

fn heading_weight(level: u8) -> FontWeight {
    match level {
        1 | 2 => FontWeight::BOLD,
        _ => FontWeight::SEMIBOLD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(chars: usize) -> ReaderBlock {
        ReaderBlock {
            kind: BlockKind::Paragraph,
            spans: vec![InlineSpan {
                text: "a".repeat(chars),
                strong: false,
                emphasis: false,
            }],
        }
    }

    #[test]
    fn estimated_block_height_tracks_reader_width() {
        // Regression: 2026-05-27. Virtual-list rows must grow when resize
        // makes text wrap to more lines, otherwise rows overlap visually.
        let block = paragraph(600);
        let wide = estimated_block_size(&block, 17.0, 780.0);
        let narrow = estimated_block_size(&block, 17.0, 320.0);

        assert!(narrow.height > wide.height);
    }

    #[test]
    fn paragraph_estimate_includes_apple_books_gap() {
        let block = paragraph(20);
        let size = estimated_block_size(&block, 17.0, 780.0);

        assert!(size.height > px(body_line_height(17.0)));
    }

    #[test]
    fn standalone_user_data_prefers_appdata_akasha() {
        let dir = standalone_user_data_dir(Some("C:\\Users\\Ada\\AppData\\Roaming".into()), || {
            PathBuf::from("D:\\repo")
        });

        assert_eq!(
            dir,
            PathBuf::from("C:\\Users\\Ada\\AppData\\Roaming").join("Akasha")
        );
    }

    #[test]
    fn standalone_user_data_falls_back_to_local_dot_dir() {
        let dir = standalone_user_data_dir(None, || PathBuf::from("D:\\repo"));

        assert_eq!(dir, PathBuf::from("D:\\repo").join(".akasha"));
    }
}

fn main() {
    let args = Args::parse();
    Application::new().run(move |cx: &mut App| {
        gpui_component::init(cx);
        let bounds = Bounds::centered(None, gpui::size(px(1060.0), px(760.0)), cx);
        let user_data_dir = args
            .user_data_dir
            .clone()
            .unwrap_or_else(default_user_data_dir);
        let open_path = args.open_path.clone();
        let dev_fps_enabled = args.dev_mode
            || std::env::var("KOSMOS_EXTENSION_DEV_MODE").ok().as_deref() == Some("1");
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Akasha".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|_| {
                    let mut app = Akasha::new(user_data_dir, dev_fps_enabled);
                    if let Some(path) = open_path {
                        app.open_book(&path);
                    }
                    app
                })
            },
        )
        .expect("failed to open Akasha window");
        cx.activate(true);
    });
}

#[derive(Debug, Default)]
struct Args {
    user_data_dir: Option<PathBuf>,
    open_path: Option<PathBuf>,
    dev_mode: bool,
}

impl Args {
    fn parse() -> Self {
        let mut parsed = Self::default();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--kosmos-user-data-dir" => {
                    parsed.user_data_dir = args.next().map(PathBuf::from);
                }
                "--open" => {
                    parsed.open_path = args.next().map(PathBuf::from);
                }
                "--kosmos-extension-id" => {
                    let _ = args.next();
                }
                "--kosmos-dev-mode" => {
                    parsed.dev_mode = true;
                }
                other if other.to_ascii_lowercase().ends_with(".epub") => {
                    parsed.open_path = Some(PathBuf::from(other));
                }
                _ => {}
            }
        }
        parsed
    }
}

fn default_user_data_dir() -> PathBuf {
    standalone_user_data_dir(std::env::var_os("APPDATA"), || {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    })
}

fn standalone_user_data_dir(
    appdata: Option<std::ffi::OsString>,
    current_dir: impl FnOnce() -> PathBuf,
) -> PathBuf {
    if let Some(appdata) = appdata {
        return PathBuf::from(appdata).join("Akasha");
    }
    current_dir().join(".akasha")
}
