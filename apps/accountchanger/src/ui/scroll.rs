use gpui::{
    AnyElement, App, AppContext, Context, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, RenderOnce, ScrollHandle, StatefulInteractiveElement, Styled, Window,
    div, point, prelude::FluentBuilder, px,
};

use crate::motion::{self, Key};
use crate::theme::ThemeExt;

const MIN_THUMB: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thumb {
    pub length: f32,
    pub offset: f32,
}

pub fn thumb(viewport: f32, max: f32, scrolled: f32) -> Option<Thumb> {
    if viewport <= 0.0 || max <= 0.5 {
        return None;
    }
    let content = viewport + max;
    let length = (viewport * viewport / content).max(MIN_THUMB.min(viewport));
    let travel = (viewport - length).max(0.0);
    Some(Thumb {
        length,
        offset: travel * (scrolled / max).clamp(0.0, 1.0),
    })
}

struct ScrollDrag {
    id: ElementId,
    grab: f32,
}

struct Ghost;

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub fn scrollbar(
    id: impl Into<ElementId>,
    handle: &ScrollHandle,
    width: f32,
    inset: f32,
    cx: &App,
) -> Option<AnyElement> {
    let id = id.into();
    let bounds = handle.bounds();
    let viewport = f32::from(bounds.size.height);
    let max = f32::from(handle.max_offset().height);
    let scrolled = -f32::from(handle.offset().y);
    let Thumb { length, offset } = thumb(viewport, max, scrolled)?;
    let p = cx.palette();
    let key = Key::new("scrollbar", &id);
    let hover = motion::value(key, cx);
    let travel = (viewport - length).max(1.0);
    let drag_handle = handle.clone();
    let own = id.clone();
    let track_id = ElementId::Name(format!("{id}-track").into());
    Some(
        div()
            .id(track_id)
            .absolute()
            .top_0()
            .bottom_0()
            .right(px(inset))
            .w(px(width))
            .on_drag_move::<ScrollDrag>(move |event, window, cx| {
                let drag = event.drag(cx);
                if drag.id != own {
                    return;
                }
                let top = f32::from(event.event.position.y - event.bounds.origin.y) - drag.grab;
                let fraction = (top / travel).clamp(0.0, 1.0);
                let current = drag_handle.offset();
                drag_handle.set_offset(point(current.x, px(-fraction * max)));
                window.refresh();
            })
            .child(
                div()
                    .id(id.clone())
                    .absolute()
                    .top(px(offset))
                    .left_0()
                    .w(px(width))
                    .h(px(length))
                    .rounded_full()
                    .bg(crate::theme::mix(p.line_strong, p.faint, hover))
                    .cursor_default()
                    .occlude()
                    .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_drag(
                        ScrollDrag {
                            id: id.clone(),
                            grab: length / 2.0,
                        },
                        |_, _, _, cx| cx.new(|_| Ghost),
                    ),
            )
            .into_any_element(),
    )
}

#[derive(IntoElement)]
pub struct ScrollArea {
    id: ElementId,
    handle: ScrollHandle,
    bar_width: f32,
    bar_inset: f32,
    content: Vec<AnyElement>,
    grow: bool,
}

impl ScrollArea {
    pub fn new(id: impl Into<ElementId>, handle: &ScrollHandle) -> Self {
        Self {
            id: id.into(),
            handle: handle.clone(),
            bar_width: 8.0,
            bar_inset: 0.0,
            content: Vec::new(),
            grow: true,
        }
    }

    pub fn fixed(mut self) -> Self {
        self.grow = false;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.content.push(child.into_any_element());
        self
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let bar_id = ElementId::Name(format!("{}-bar", self.id).into());
        let bar = scrollbar(bar_id, &self.handle, self.bar_width, self.bar_inset, cx);
        div()
            .relative()
            .flex()
            .flex_col()
            .min_h_0()
            .when(self.grow, |area| area.flex_1())
            .child(
                div()
                    .id(self.id)
                    .flex()
                    .flex_col()
                    .size_full()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.handle)
                    .children(self.content),
            )
            .children(bar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_content_has_no_thumb() {
        assert!(thumb(400.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn thumb_reaches_bottom() {
        let t = thumb(400.0, 600.0, 600.0).unwrap();
        assert!((t.offset + t.length - 400.0).abs() < 1e-3);
        assert_eq!(thumb(400.0, 600.0, 0.0).unwrap().offset, 0.0);
    }
}
