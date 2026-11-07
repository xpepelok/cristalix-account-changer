use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, App, AppContext, Bounds, Context, ElementId, FontWeight, Hsla,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Radians, Render,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Transformation, Window, canvas,
    div, prelude::FluentBuilder, px, relative,
};

use crate::assets::{Icon, icon};
use crate::motion::{self, EASE_OUT, Key, cubic_bezier};
use crate::sound::{self, Sound};
use crate::theme::{Palette, ThemeExt, alpha, mix, parse_hex, rem, rgba, shadow};
use crate::ui::spaced;

type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

pub fn chip_colors(color: &str, p: &Palette) -> (Hsla, Hsla) {
    let tone = parse_hex(color).unwrap_or(p.muted);
    (tone, alpha(tone, 0.16))
}

#[derive(IntoElement)]
pub struct Chip {
    label: SharedString,
    color: SharedString,
    compact: bool,
}

impl Chip {
    pub fn new(label: impl Into<SharedString>, color: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            color: color.into(),
            compact: false,
        }
    }

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let (fg, bg) = chip_colors(&self.color, &p);
        let (size, pad_x, pad_y) = if self.compact {
            (0.55, 5.0, 1.0)
        } else {
            (0.6, 7.0, 2.0)
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .px(px(pad_x))
            .py(px(pad_y))
            .rounded(px(5.0))
            .bg(bg)
            .text_color(fg)
            .text_size(rem(size))
            .font_weight(FontWeight::EXTRA_BOLD)
            .child(spaced(&self.label.to_uppercase(), 0.4))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeTone {
    Ready,
    Expired,
    Missing,
    Online,
    Offline,
}

#[derive(IntoElement)]
pub struct Badge {
    text: SharedString,
    tone: BadgeTone,
    size: Option<gpui::Pixels>,
}

impl Badge {
    pub fn new(text: impl Into<SharedString>, tone: BadgeTone) -> Self {
        Self {
            text: text.into(),
            tone,
            size: None,
        }
    }

    pub fn size(mut self, size: gpui::Pixels) -> Self {
        self.size = Some(size);
        self
    }

    pub fn status(text: impl Into<SharedString>, on: bool) -> Self {
        Self::new(
            text,
            if on {
                BadgeTone::Online
            } else {
                BadgeTone::Offline
            },
        )
    }

    pub fn missing() -> Self {
        Self::new("нет токена", BadgeTone::Missing)
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let (fg, bg) = match self.tone {
            BadgeTone::Ready | BadgeTone::Online => (p.green, rgba(0x1fa344, 0.14)),
            BadgeTone::Expired => (p.yellow, rgba(0xe3a400, 0.14)),
            BadgeTone::Missing => (p.faint, p.surface_2),
            BadgeTone::Offline => (p.muted, alpha(p.muted, 0.16)),
        };
        let status = matches!(self.tone, BadgeTone::Online | BadgeTone::Offline);
        let base = div()
            .min_w_0()
            .px(px(7.0))
            .py(px(2.0))
            .rounded_full()
            .bg(bg)
            .text_color(fg)
            .font_weight(FontWeight::BOLD);
        if status {
            base.text_size(self.size.unwrap_or(rem(0.66)))
                .flex_none()
                .whitespace_nowrap()
                .overflow_hidden()
                .child(self.text)
        } else {
            base.flex_none()
                .text_size(rem(0.6))
                .child(spaced(&self.text.to_uppercase(), 0.4))
        }
    }
}

#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    key: Key,
    on: bool,
    on_toggle: Option<ToggleHandler>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>, key: Key, on: bool) -> Self {
        Self {
            id: id.into(),
            key,
            on,
            on_toggle: None,
        }
    }

    pub fn on_toggle(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let key = self.key;
        if motion::is_on(key, cx) != self.on {
            motion::drive(
                key,
                self.on,
                Duration::from_millis(180),
                EASE_OUT,
                window,
                cx,
            );
        }
        let t = motion::value(key, cx);
        let on = self.on;
        let handler = self.on_toggle;
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .w(px(38.0))
            .h(px(22.0))
            .rounded_full()
            .bg(mix(p.line_strong, p.blue, t))
            .cursor_pointer()
            .occlude()
            .when_some(handler, |toggle, handler| {
                toggle.on_click(move |_, window, cx| {
                    sound::play(Sound::Tap);
                    handler(&!on, window, cx);
                })
            })
            .child(
                div()
                    .absolute()
                    .top(px(3.0))
                    .left(px(3.0 + 16.0 * t))
                    .size(px(16.0))
                    .rounded_full()
                    .bg(p.white)
                    .shadow(shadow(0.0, 1.0, 3.0, 0.0, rgba(0x000000, 0.4))),
            )
    }
}

#[derive(IntoElement)]
pub struct Checkbox {
    on: bool,
    color: Option<Hsla>,
}

impl Checkbox {
    pub fn new(on: bool) -> Self {
        Self { on, color: None }
    }

    pub fn color(mut self, color: Option<Hsla>) -> Self {
        self.color = color;
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let fill = self.color.unwrap_or(p.blue);
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(px(18.0))
            .rounded(px(5.0))
            .border(px(1.5))
            .border_color(if self.on { fill } else { p.line_strong })
            .when(self.on, |b| {
                b.bg(fill).child(icon(Icon::Check, px(13.0), p.white))
            })
    }
}

#[derive(IntoElement)]
pub struct Spinner {
    id: ElementId,
    size: f32,
}

impl Spinner {
    pub fn new(id: impl Into<ElementId>, size: f32) -> Self {
        Self {
            id: id.into(),
            size,
        }
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let size = px(self.size);
        div()
            .relative()
            .flex_none()
            .size(size)
            .child(icon(Icon::SpinnerRing, size, p.line_strong).absolute())
            .child(
                icon(Icon::SpinnerArc, size, p.blue)
                    .absolute()
                    .with_animation(
                        self.id,
                        Animation::new(Duration::from_millis(700)).repeat(),
                        |svg, t| {
                            svg.with_transformation(Transformation::rotate(Radians(
                                t * std::f32::consts::TAU,
                            )))
                        },
                    ),
            )
    }
}

#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    options: Vec<SharedString>,
    selected: usize,
    width: f32,
    on_select: Option<SelectHandler>,
}

impl Segmented {
    pub fn new(id: impl Into<ElementId>, options: Vec<SharedString>, selected: usize) -> Self {
        Self {
            id: id.into(),
            options,
            selected,
            width: 246.0,
            on_select: None,
        }
    }

    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

struct SegmentState {
    current: usize,
    from: usize,
    since: Option<Instant>,
}

const SEGMENT_SLIDE: Duration = Duration::from_millis(240);

impl RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let count = self.options.len().max(1) as f32;
        let slot = (self.width - 8.0) / count;
        let selected = self.selected;
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| SegmentState {
            current: selected,
            from: selected,
            since: None,
        });
        if state.read(cx).current != selected {
            state.update(cx, |state, _| {
                state.from = state.current;
                state.current = selected;
                state.since = Some(Instant::now());
            });
        }
        let (from, since) = {
            let state = state.read(cx);
            (state.from, state.since)
        };
        let t = since.map_or(1.0, |since| {
            let k = since.elapsed().as_secs_f32() / SEGMENT_SLIDE.as_secs_f32();
            if k < 1.0 {
                window.request_animation_frame();
            }
            cubic_bezier(EASE_OUT, k.min(1.0))
        });
        let at = from as f32 + (selected as f32 - from as f32) * t;
        let handler = self.on_select;
        div()
            .id(self.id)
            .relative()
            .flex()
            .flex_none()
            .w(px(self.width))
            .p(px(3.0))
            .bg(p.surface_2)
            .border_1()
            .border_color(p.line)
            .rounded(px(9.0))
            .child(
                div()
                    .absolute()
                    .top(px(3.0))
                    .bottom(px(3.0))
                    .left(px(3.0 + at * slot))
                    .w(px(slot))
                    .rounded(px(6.0))
                    .bg(p.blue),
            )
            .children(self.options.into_iter().enumerate().map(|(index, label)| {
                let on = index == selected;
                let handler = handler.clone();
                div()
                    .id(index)
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .justify_center()
                    .py(px(5.0))
                    .px(px(4.0))
                    .rounded(px(6.0))
                    .text_size(rem(0.72))
                    .font_weight(FontWeight::SEMIBOLD)
                    .whitespace_nowrap()
                    .text_color(if on { p.white } else { p.muted })
                    .cursor_pointer()
                    .when(!on, |b| b.hover(|s| s.text_color(p.text)))
                    .when_some(handler, |b, handler| {
                        b.on_click(move |_, window, cx| {
                            sound::play(Sound::Tap);
                            handler(&index, window, cx);
                        })
                    })
                    .child(label)
            }))
    }
}

struct SliderDrag(ElementId);

struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(Default)]
struct SliderState {
    bounds: Option<Bounds<Pixels>>,
    pending: Option<u32>,
}

type SliderHandler = Rc<dyn Fn(&u32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    value: u32,
    min: u32,
    max: u32,
    step: u32,
    disabled: bool,
    track: Option<Hsla>,
    on_input: Option<SliderHandler>,
    on_commit: Option<SliderHandler>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: u32, min: u32, max: u32, step: u32) -> Self {
        Self {
            id: id.into(),
            value,
            min,
            max,
            step,
            disabled: false,
            track: None,
            on_input: None,
            on_commit: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn track(mut self, color: Hsla) -> Self {
        self.track = Some(color);
        self
    }

    pub fn on_input(mut self, handler: impl Fn(&u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_input = Some(Rc::new(handler));
        self
    }

    pub fn on_commit(mut self, handler: impl Fn(&u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_commit = Some(Rc::new(handler));
        self
    }
}

fn slider_value(x: Pixels, bounds: Bounds<Pixels>, min: u32, max: u32, step: u32) -> u32 {
    let width = f32::from(bounds.size.width).max(1.0);
    let fraction = (f32::from(x - bounds.origin.x) / width).clamp(0.0, 1.0);
    let raw = min as f32 + fraction * (max - min) as f32;
    let stepped = ((raw - min as f32) / step as f32).round() as u32 * step + min;
    stepped.clamp(min, max)
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| SliderState::default());
        let fraction =
            (self.value.saturating_sub(self.min)) as f32 / (self.max - self.min).max(1) as f32;
        let (min, max, step) = (self.min, self.max, self.step);
        let id = self.id.clone();
        let track = div()
            .id(self.id.clone())
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(16.0))
            .flex()
            .items_center()
            .child(
                canvas(
                    {
                        let state = state.clone();
                        move |bounds, _, cx| {
                            state.update(cx, |state, _| state.bounds = Some(bounds));
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .w_full()
                    .h(px(6.0))
                    .rounded_full()
                    .bg(self.track.unwrap_or(p.surface_2)),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(fraction))
                    .ml(px(-8.0 + 16.0 * (0.5 - fraction)))
                    .size(px(16.0))
                    .rounded_full()
                    .bg(p.blue)
                    .border_2()
                    .border_color(p.surface)
                    .shadow(shadow(0.0, 0.0, 0.0, 1.0, p.blue)),
            );
        if self.disabled {
            return track.opacity(0.35);
        }
        let input = self.on_input;
        let commit = self.on_commit;
        let pending_commit = {
            let state = state.clone();
            let commit = commit.clone();
            move |window: &mut Window, cx: &mut App| {
                let pending = state.update(cx, |state, _| state.pending.take());
                if let (Some(value), Some(commit)) = (pending, commit.as_ref()) {
                    commit(&value, window, cx);
                }
            }
        };
        let pending_commit = Rc::new(pending_commit);
        track
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                let input = input.clone();
                move |event, window, cx| {
                    let Some(bounds) = state.read(cx).bounds else {
                        return;
                    };
                    let value = slider_value(event.position.x, bounds, min, max, step);
                    state.update(cx, |state, _| state.pending = Some(value));
                    if let Some(input) = input.as_ref() {
                        input(&value, window, cx);
                    }
                }
            })
            .on_drag(SliderDrag(id), |_, _, _, cx| cx.new(|_| DragGhost))
            .on_drag_move::<SliderDrag>({
                let state = state.clone();
                let own = self.id.clone();
                move |event, window, cx| {
                    if event.drag(cx).0 != own {
                        return;
                    }
                    let value = slider_value(event.event.position.x, event.bounds, min, max, step);
                    state.update(cx, |state, _| state.pending = Some(value));
                    if let Some(input) = input.as_ref() {
                        input(&value, window, cx);
                    }
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let pending_commit = pending_commit.clone();
                move |_, window, cx| pending_commit(window, cx)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
                pending_commit(window, cx)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    #[test]
    fn slider_snaps_to_steps() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(10.0)));
        assert_eq!(slider_value(px(0.0), bounds, 1024, 32768, 512), 1024);
        assert_eq!(slider_value(px(100.0), bounds, 1024, 32768, 512), 32768);
        assert_eq!(slider_value(px(-50.0), bounds, 1024, 32768, 512), 1024);
        assert_eq!(slider_value(px(50.0), bounds, 0, 100, 10), 50);
    }
}
