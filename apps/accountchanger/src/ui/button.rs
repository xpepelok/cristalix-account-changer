use std::time::Duration;

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Div, ElementId, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Radians, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Transformation, Window, div, prelude::FluentBuilder, px,
    relative,
};

use crate::assets::{Icon, icon};
use crate::motion::{self, EASE, EASE_SPRING, Key};
use crate::sound::{self, Sound};
use crate::theme::{Palette, ThemeExt, alpha, mix, rem, rgb, rgba, shadow};
use crate::ui::{ClickHandler, with_tooltip};

const PRESS_IN: Duration = Duration::from_millis(70);
const SPRING: Duration = Duration::from_millis(160);

pub fn press_key(key: Key) -> Key {
    Key::new("press", key)
}

fn press_layer(
    press: f32,
    depth: f32,
    radius: f32,
    bg: Hsla,
    border: Hsla,
    glow: Vec<BoxShadow>,
) -> Div {
    let inset = relative(depth * press);
    div()
        .absolute()
        .top(inset)
        .bottom(inset)
        .left(inset)
        .right(inset)
        .rounded(px(radius * (1.0 - depth * press)))
        .border_1()
        .border_color(border)
        .bg(bg)
        .shadow(glow)
}

pub fn pressable<E: InteractiveElement + 'static>(element: E, key: Key) -> E {
    element
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            motion::drive(press_key(key), true, PRESS_IN, EASE, window, cx);
        })
        .on_mouse_up(MouseButton::Left, move |_, window, cx| {
            motion::drive(press_key(key), false, SPRING, EASE_SPRING, window, cx);
        })
        .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
            motion::drive(press_key(key), false, SPRING, EASE_SPRING, window, cx);
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Ghost,
    Outline,
    Stop,
    Danger,
    DangerGhost,
    Tinted,
}

struct Look {
    bg: Hsla,
    fg: Hsla,
    border: Hsla,
}

fn button_look(style: ButtonStyle, p: &Palette, t: f32) -> (Look, Vec<gpui::BoxShadow>) {
    let lerp = |a: Look, b: Look| Look {
        bg: mix(a.bg, b.bg, t),
        fg: mix(a.fg, b.fg, t),
        border: mix(a.border, b.border, t),
    };
    let look = |bg, fg, border| Look { bg, fg, border };
    let clear = alpha(p.surface, 0.0);
    let (rest, hover, glow) = match style {
        ButtonStyle::Primary => (
            look(p.blue, p.white, p.blue),
            look(p.blue_hover, p.white, p.blue_hover),
            Some(shadow(0.0, 6.0, 18.0, -7.0, alpha(p.blue, t))),
        ),
        ButtonStyle::Tinted => (
            look(p.blue, p.white, p.blue),
            look(p.blue_hover, p.white, p.blue_hover),
            None,
        ),
        ButtonStyle::Ghost => (
            look(p.surface, p.text, p.line),
            look(p.surface_2, p.text, p.line_strong),
            Some(shadow(0.0, 5.0, 14.0, -8.0, rgba(0x000000, 0.55 * t))),
        ),
        ButtonStyle::Outline => (
            look(clear, p.muted, p.line_strong),
            look(clear, p.text, p.blue),
            Some(shadow(0.0, 5.0, 14.0, -8.0, rgba(0x000000, 0.55 * t))),
        ),
        ButtonStyle::Stop => (
            look(p.stop, p.white, p.stop),
            look(p.red, p.white, p.red),
            Some(shadow(0.0, 6.0, 18.0, -7.0, alpha(p.stop, t))),
        ),
        ButtonStyle::Danger => (
            look(p.red, p.white, p.red),
            look(rgb(0xc9463f), p.white, p.red),
            None,
        ),
        ButtonStyle::DangerGhost => (
            look(clear, p.faint, p.line),
            look(clear, p.red, alpha(p.red, 0.5)),
            None,
        ),
    };
    (lerp(rest, hover), glow.unwrap_or_default())
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    key: Key,
    label: SharedString,
    style: ButtonStyle,
    icon: Option<(Icon, f32)>,
    disabled: bool,
    full: bool,
    grow: bool,
    min_width: Option<f32>,
    height: Option<f32>,
    tooltip: Option<SharedString>,
    sound: Option<Sound>,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: &'static str, label: impl Into<SharedString>, style: ButtonStyle) -> Self {
        Self {
            id: id.into(),
            key: Key::of(id),
            label: label.into(),
            style,
            icon: None,
            disabled: false,
            full: false,
            grow: false,
            min_width: None,
            height: None,
            tooltip: None,
            sound: Some(
                if style == ButtonStyle::Primary || style == ButtonStyle::Tinted {
                    Sound::Confirm
                } else {
                    Sound::Tap
                },
            ),
            on_click: None,
        }
    }

    pub fn icon(mut self, icon: Icon, size: f32) -> Self {
        self.icon = Some((icon, size));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn full(mut self) -> Self {
        self.full = true;
        self
    }

    pub fn grow(mut self) -> Self {
        self.grow = true;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let key = self.key;
        let t = if self.disabled {
            0.0
        } else {
            motion::value(key, cx)
        };
        let (look, glow) = button_look(self.style, &p, t);
        let lift = if self.disabled {
            0.0
        } else {
            -cubic_lift(key, cx)
        };
        let sound = self.sound;
        let press = if self.disabled {
            0.0
        } else {
            motion::value(press_key(key), cx)
        };
        let mut button = div()
            .id(self.id)
            .relative()
            .top(px(lift * (1.0 - press)))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(7.0))
            .h(px(self.height.unwrap_or(41.0)))
            .px(px(16.0))
            .child(press_layer(press, 0.03, 10.0, look.bg, look.border, glow))
            .text_color(look.fg)
            .text_size(rem(0.85))
            .font_weight(FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .when_some(self.min_width, |b, w| b.min_w(px(w)))
            .when(self.full, |b| b.w_full())
            .when(self.grow, |b| b.flex_1())
            .when_some(self.icon, |b, (glyph, size)| {
                b.child(icon(glyph, px(size), look.fg))
            })
            .child(self.label);
        if self.disabled {
            button = button.opacity(0.4);
        } else {
            button = pressable(button, key).cursor_pointer().occlude().on_hover(
                move |hovered, window, cx| {
                    motion::hover(key, *hovered, window, cx);
                    motion::drive(lift_key(key), *hovered, SPRING, EASE_SPRING, window, cx);
                },
            );
            if let Some(handler) = self.on_click {
                button = button.on_click(move |event, window, cx| {
                    if let Some(sound) = sound {
                        sound::play(sound);
                    }
                    handler(event, window, cx);
                });
            }
        }
        match self.tooltip {
            Some(text) => with_tooltip(button, key, text).into_any_element(),
            None => button.into_any_element(),
        }
    }
}

fn lift_key(key: Key) -> Key {
    Key::new("lift", key)
}

fn cubic_lift(key: Key, cx: &App) -> f32 {
    motion::value(lift_key(key), cx)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconStyle {
    Round,
    Window,
    WindowClose,
    Square,
    Danger,
    Outline,
    Filter,
    Play,
    Stop,
    Busy,
    Close,
}

impl IconStyle {
    fn size(self) -> (f32, f32) {
        match self {
            Self::Round => (36.0, 9.0),
            Self::Window | Self::WindowClose => (34.0, 8.0),
            Self::Square => (42.0, 10.0),
            Self::Danger | Self::Filter => (40.0, 10.0),
            Self::Outline => (41.0, 10.0),
            Self::Play | Self::Stop | Self::Busy => (40.0, 12.0),
            Self::Close => (30.0, 8.0),
        }
    }

    fn lifts(self) -> bool {
        matches!(
            self,
            Self::Round | Self::Square | Self::Outline | Self::Filter
        )
    }

    fn look(self, p: &Palette, t: f32, active: bool) -> Look {
        let clear = alpha(p.surface, 0.0);
        let t = if active { 1.0 } else { t };
        let pair = |rest: (Hsla, Hsla, Hsla), hover: (Hsla, Hsla, Hsla)| Look {
            bg: mix(rest.0, hover.0, t),
            fg: mix(rest.1, hover.1, t),
            border: mix(rest.2, hover.2, t),
        };
        match self {
            Self::Round => pair(
                (clear, p.muted, p.line),
                (p.surface_2, p.text, p.line_strong),
            ),
            Self::Window => pair((clear, p.muted, clear), (p.surface_2, p.text, clear)),
            Self::WindowClose => pair((clear, p.muted, clear), (p.stop, p.white, clear)),
            Self::Square => pair(
                (p.surface_2, p.muted, p.line),
                (
                    p.icon_hover_bg,
                    p.icon_hover_fg,
                    if p.is_light() { p.blue } else { p.line_strong },
                ),
            ),
            Self::Danger => pair((clear, p.red, alpha(p.red, 0.4)), (p.red, p.white, p.red)),
            Self::Outline => pair((clear, p.muted, p.line_strong), (clear, p.text, p.blue)),
            Self::Filter => pair(
                (p.surface_2, p.muted, p.line),
                (p.surface_2, p.text, p.line_strong),
            ),
            Self::Play => pair(
                (p.surface_2, p.text, p.line_strong),
                (p.blue, p.white, p.blue),
            ),
            Self::Stop => pair((p.stop, p.white, p.stop), (p.red, p.white, p.red)),
            Self::Busy => Look {
                bg: p.surface_2,
                fg: p.muted,
                border: p.line_strong,
            },
            Self::Close => pair(
                (p.surface_2, p.muted, clear),
                (p.close_hover_bg, p.close_hover_fg, clear),
            ),
        }
    }
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    key: Key,
    style: IconStyle,
    icon: Option<(Icon, f32)>,
    content: Option<AnyElement>,
    rotate_on_hover: Option<(f32, Duration, [f32; 4])>,
    glyph_transform: Option<Transformation>,
    pulse: Option<(Key, Duration)>,
    on_secondary: Option<ClickHandler>,
    active: bool,
    disabled: bool,
    size: Option<(f32, f32)>,
    tooltip: Option<SharedString>,
    sound: Option<Sound>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, key: Key, style: IconStyle) -> Self {
        Self {
            id: id.into(),
            key,
            style,
            icon: None,
            content: None,
            rotate_on_hover: None,
            glyph_transform: None,
            pulse: None,
            on_secondary: None,
            active: false,
            disabled: false,
            size: None,
            tooltip: None,
            sound: Some(Sound::Tap),
            on_click: None,
        }
    }

    pub fn icon(mut self, icon: Icon, size: f32) -> Self {
        self.icon = Some((icon, size));
        self
    }

    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn rotate_on_hover(mut self, degrees: f32, duration: Duration, curve: [f32; 4]) -> Self {
        self.rotate_on_hover = Some((degrees, duration, curve));
        self
    }

    pub fn glyph_transform(mut self, transform: Transformation) -> Self {
        self.glyph_transform = Some(transform);
        self
    }

    pub fn pulse(mut self, key: Key, duration: Duration) -> Self {
        self.pulse = Some((key, duration));
        self
    }

    pub fn on_secondary(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_secondary = Some(Box::new(handler));
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn sound(mut self, sound: Option<Sound>) -> Self {
        self.sound = sound;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let key = self.key;
        let t = if self.disabled {
            0.0
        } else {
            motion::value(key, cx)
        };
        let look = self.style.look(&p, t, self.active);
        let (side, radius) = self.style.size();
        let (width, height) = self.size.unwrap_or((side, side));
        let lifts = self.style.lifts() && !self.disabled;
        let lift = if lifts { -cubic_lift(key, cx) } else { 0.0 };
        let spin_key = Key::new("spin", key);
        let spin = self
            .rotate_on_hover
            .map(|(degrees, _, _)| degrees * motion::value(spin_key, cx));
        let sound = self.sound;
        let rotation = self.rotate_on_hover;
        let pulse = self.pulse;
        let transform = self.glyph_transform;
        let glyph = self.icon.map(|(glyph, size)| {
            let svg = icon(glyph, px(size), look.fg);
            match (spin, transform) {
                (_, Some(transform)) => svg.with_transformation(transform).into_any_element(),
                (Some(degrees), None) => svg
                    .with_transformation(Transformation::rotate(Radians(degrees.to_radians())))
                    .into_any_element(),
                (None, None) => svg.into_any_element(),
            }
        });
        let pressing = !self.disabled && self.style != IconStyle::Busy;
        let press = if pressing {
            motion::value(press_key(key), cx)
        } else {
            0.0
        };
        let mut button = div()
            .id(self.id)
            .relative()
            .top(px(lift * (1.0 - press)))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(width))
            .h(px(height))
            .child(press_layer(
                press,
                0.05,
                radius,
                look.bg,
                look.border,
                Vec::new(),
            ))
            .text_color(look.fg)
            .children(glyph)
            .children(self.content);
        if self.disabled {
            button = button.opacity(0.5);
        } else if self.style != IconStyle::Busy {
            button = pressable(button, key).cursor_pointer().occlude().on_hover(
                move |hovered, window, cx| {
                    motion::hover(key, *hovered, window, cx);
                    if lifts {
                        motion::drive(lift_key(key), *hovered, SPRING, EASE_SPRING, window, cx);
                    }
                    if let Some((_, duration, curve)) = rotation {
                        motion::drive(spin_key, *hovered, duration, curve, window, cx);
                    }
                    if let Some((pulse, duration)) = pulse {
                        if *hovered {
                            motion::restart(pulse, duration, window, cx);
                        } else {
                            motion::stop(pulse, window, cx);
                        }
                    }
                },
            );
            if let Some(handler) = self.on_secondary {
                button = button.on_mouse_down(MouseButton::Right, move |event, window, cx| {
                    let click = ClickEvent::Mouse(gpui::MouseClickEvent {
                        down: event.clone(),
                        up: gpui::MouseUpEvent {
                            button: MouseButton::Right,
                            position: event.position,
                            modifiers: event.modifiers,
                            click_count: 1,
                        },
                    });
                    handler(&click, window, cx);
                });
            }
            if let Some(handler) = self.on_click {
                button = button.on_click(move |event, window, cx| {
                    if let Some(sound) = sound {
                        sound::play(sound);
                    }
                    handler(event, window, cx);
                });
            }
        } else {
            button = button.occlude();
        }
        match self.tooltip {
            Some(text) => with_tooltip(button, key, text).into_any_element(),
            None => button.into_any_element(),
        }
    }
}
