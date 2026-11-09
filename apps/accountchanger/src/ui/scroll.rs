use std::collections::HashMap;
use std::time::Instant;

use gpui::{
    AnyElement, App, AppContext, Context, DispatchPhase, ElementId, Global, HitboxBehavior,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, RenderOnce, ScrollDelta,
    ScrollHandle, ScrollWheelEvent, StatefulInteractiveElement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder, px,
};

use crate::motion::{self, Key};
use crate::theme::ThemeExt;

const MIN_THUMB: f32 = 24.0;
const GLIDE: f32 = 15.0;
const GLIDE_STEP: f32 = 0.05;
const SETTLE: f32 = 0.5;

struct Glide {
    target: f32,
    last: Instant,
}

#[derive(Default)]
struct Glides(HashMap<ElementId, Glide>);

impl Global for Glides {}

fn advance(id: &ElementId, handle: &ScrollHandle, window: &mut Window, cx: &mut App) {
    if !cx
        .try_global::<Glides>()
        .is_some_and(|glides| glides.0.contains_key(id))
    {
        return;
    }
    let glides = cx.global_mut::<Glides>();
    let Some(glide) = glides.0.get_mut(id) else {
        return;
    };
    let now = Instant::now();
    let step = now.duration_since(glide.last).as_secs_f32().min(GLIDE_STEP);
    glide.last = now;
    let max = f32::from(handle.max_offset().height);
    glide.target = glide.target.clamp(0.0, max);
    let offset = handle.offset();
    let current = -f32::from(offset.y);
    let gap = glide.target - current;
    if gap.abs() < SETTLE {
        handle.set_offset(point(offset.x, px(-glide.target)));
        glides.0.remove(id);
        return;
    }
    let next = current + gap * (1.0 - (-GLIDE * step).exp());
    handle.set_offset(point(offset.x, px(-next)));
    window.request_animation_frame();
}

fn wheel(id: ElementId, handle: ScrollHandle) -> impl IntoElement {
    let bounds_handle = handle.clone();
    canvas(
        move |_, window, _| window.insert_hitbox(bounds_handle.bounds(), HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || !hitbox.should_handle_scroll(window) {
                    return;
                }
                let ScrollDelta::Lines(lines) = event.delta else {
                    if let Some(glides) = cx.try_global::<Glides>()
                        && glides.0.contains_key(&id)
                    {
                        cx.global_mut::<Glides>().0.remove(&id);
                    }
                    return;
                };
                let max = f32::from(handle.max_offset().height);
                if max <= SETTLE || lines.y == 0.0 {
                    return;
                }
                let current = -f32::from(handle.offset().y);
                let distance = lines.y * f32::from(window.line_height());
                let glide = cx
                    .default_global::<Glides>()
                    .0
                    .entry(id.clone())
                    .or_insert(Glide {
                        target: current,
                        last: Instant::now(),
                    });
                glide.target = (glide.target - distance).clamp(0.0, max);
                cx.stop_propagation();
                window.refresh();
            });
        },
    )
    .absolute()
    .size_0()
}

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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        advance(&self.id, &self.handle, window, cx);
        let bar_id = ElementId::Name(format!("{}-bar", self.id).into());
        let bar = scrollbar(bar_id, &self.handle, self.bar_width, self.bar_inset, cx);
        let wheel = wheel(self.id.clone(), self.handle.clone());
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
                    .child(wheel)
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
