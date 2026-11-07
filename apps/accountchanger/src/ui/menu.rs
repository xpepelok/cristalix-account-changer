use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, Div, ElementId, FontWeight, Hsla, InteractiveElement,
    IntoElement, Stateful, Styled, div, px,
};

use crate::motion::EASE;
use crate::theme::{Palette, ThemeExt};

pub fn menu_surface(background: Hsla, radius: f32, padding: f32, cx: &App) -> Div {
    let p = cx.palette();
    div()
        .flex()
        .flex_col()
        .p(px(padding))
        .rounded(px(radius))
        .border_1()
        .border_color(p.line_strong)
        .bg(background)
        .shadow(p.popup_shadow())
}

pub fn drop_in(id: impl Into<ElementId>, menu: Stateful<Div>, closing: Option<f32>) -> AnyElement {
    if let Some(progress) = closing {
        let t = crate::motion::cubic_bezier(crate::motion::EASE_IN, progress);
        return menu
            .occlude()
            .opacity(1.0 - t)
            .mt(px(-5.0 * t))
            .into_any_element();
    }
    menu.occlude()
        .with_animation(
            id,
            Animation::new(Duration::from_millis(140)).with_easing(crate::motion::eased(EASE)),
            |menu, t| menu.opacity(t).mt(px(-4.0 * (1.0 - t))),
        )
        .into_any_element()
}

pub fn pop_in(id: impl Into<ElementId>, menu: Stateful<Div>) -> AnyElement {
    menu.occlude()
        .with_animation(
            id,
            Animation::new(Duration::from_millis(140)).with_easing(crate::motion::eased(EASE)),
            |menu, t| menu.mt(px(-6.0 * (1.0 - t))),
        )
        .into_any_element()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Plain,
    Active,
    Danger,
    Action,
}

pub fn menu_item(id: impl Into<ElementId>, kind: MenuItem, p: &Palette) -> Stateful<Div> {
    let (color, hover_bg, bg) = match kind {
        MenuItem::Plain => (p.text, p.wash, None),
        MenuItem::Active => (
            p.active_text,
            p.wash,
            Some(gpui::Hsla { a: 0.16, ..p.blue }),
        ),
        MenuItem::Danger => (p.red, gpui::Hsla { a: 0.14, ..p.red }, None),
        MenuItem::Action => (p.muted, p.wash, None),
    };
    let hover_fg = if kind == MenuItem::Action {
        p.text
    } else {
        color
    };
    let mut item = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .rounded(px(8.0))
        .text_color(color)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg));
    if kind == MenuItem::Action {
        item = item.font_weight(FontWeight::SEMIBOLD);
    }
    if let Some(bg) = bg {
        item = item.bg(bg);
    }
    item
}

pub fn menu_separator(p: &Palette, margin_x: f32, margin_y: f32) -> Div {
    div()
        .h(px(1.0))
        .mx(px(margin_x))
        .my(px(margin_y))
        .bg(p.line)
        .flex_none()
}
