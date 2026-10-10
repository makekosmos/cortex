//! Manager button typography. GPUI's internal Medium label is 16px and ignores
//! the page's inherited font size. Render a 12.5px caption inside the existing
//! Imago/GPUI control instead: geometry, variants, focus and behavior stay intact.
use ::gpui::{prelude::*, *};
use gpui_component::{button::Button as ComponentButton, Disableable};

pub use imago_gpui::button::ButtonKind;
pub const LABEL_SIZE: f32 = 12.5;
pub const LABEL_LINE_HEIGHT: f32 = 16.0;

#[derive(IntoElement)]
pub struct Button {
    inner: ComponentButton,
    label: Option<SharedString>,
    accessible_name: Option<SharedString>,
}

impl Button {
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn accessibility_label(mut self, name: impl Into<SharedString>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    pub fn on_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.inner = self.inner.on_click(listener);
        self
    }

    // NOTE: no `hover` forwarding — gpui-component's Button::render applies the
    // variant hover itself, so a user-set hover_style would collide (it was
    // silently overwritten in release builds and panics under debug_assertions).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner = self.inner.disabled(disabled);
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

fn caption(text: SharedString) -> Div {
    div()
        .min_w_0()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .text_size(crate::theme::ui_px(LABEL_SIZE))
        .line_height(crate::theme::ui_px(LABEL_LINE_HEIGHT))
        .child(text)
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut inner = self.inner;
        if let Some(name) = self.accessible_name.or_else(|| self.label.clone()) {
            inner = inner.accessibility_label(name);
        }
        if let Some(text) = self.label {
            inner = inner.child(
                caption(text)
                    .id("manager-button-caption")
                    .debug_selector(|| "manager-button-caption".into()),
            );
        }
        inner
    }
}

pub fn button(id: impl Into<ElementId>, kind: ButtonKind) -> Button {
    Button {
        inner: imago_gpui::button::button(id, kind),
        label: None,
        accessible_name: None,
    }
}

pub fn primary(id: impl Into<ElementId>) -> Button {
    button(id, ButtonKind::Primary)
}
pub fn secondary(id: impl Into<ElementId>) -> Button {
    button(id, ButtonKind::Secondary)
}
pub fn ghost(id: impl Into<ElementId>) -> Button {
    button(id, ButtonKind::Ghost)
}
pub fn danger(id: impl Into<ElementId>) -> Button {
    button(id, ButtonKind::Danger)
}

pub fn btn<T: 'static>(
    id: &'static str,
    label: &'static str,
    primary: bool,
    cx: &mut Context<T>,
    on_click: impl Fn(&mut T, &mut Context<T>) + 'static,
) -> Button {
    button(
        id,
        if primary {
            ButtonKind::Primary
        } else {
            ButtonKind::Ghost
        },
    )
    .label(label)
    .on_click(cx.listener(move |this, _, _, cx| {
        on_click(this, cx);
        cx.notify();
    }))
}

pub fn btn_id(
    id: &str,
    label: &'static str,
    listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    ghost(SharedString::from(id.to_owned()))
        .label(label)
        .on_click(listener)
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod tests;
