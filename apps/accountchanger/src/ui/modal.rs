use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, Div, ElementId, FontWeight, InteractiveElement,
    IntoElement, MouseButton, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::assets::Icon;
use crate::motion::{EASE, EASE_OUT, Key, cubic_bezier};
use crate::theme::{ThemeExt, rem};
use crate::ui::{IconButton, IconStyle};

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

pub fn modal_head(title: impl Into<SharedString>, subtitle: Option<SharedString>, cx: &App) -> Div {
    let p = cx.palette();
    div()
        .flex_none()
        .flex()
        .flex_col()
        .pt(px(20.0))
        .px(px(20.0))
        .pb(px(14.0))
        .border_b_1()
        .border_color(p.line)
        .child(
            div()
                .pr(px(40.0))
                .text_size(rem(1.15))
                .font_weight(FontWeight::BOLD)
                .child(title.into()),
        )
        .when_some(subtitle, |head, subtitle| {
            head.child(
                div()
                    .mt(px(3.0))
                    .pr(px(40.0))
                    .text_size(rem(0.78))
                    .text_color(p.muted)
                    .child(subtitle),
            )
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entrance {
    Pop,
    Unfold,
}

#[derive(IntoElement)]
pub struct ModalFrame {
    id: ElementId,
    width: f32,
    max_width_fraction: f32,
    padding: (f32, f32, f32, f32),
    entrance: Entrance,
    close: bool,
    overflow_visible: bool,
    on_close: CloseHandler,
    children: Vec<AnyElement>,
}

impl ModalFrame {
    pub fn new(
        id: impl Into<ElementId>,
        width: f32,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            width,
            max_width_fraction: 0.92,
            padding: (0.0, 0.0, 0.0, 0.0),
            entrance: Entrance::Pop,
            close: true,
            overflow_visible: false,
            on_close: Rc::new(on_close),
            children: Vec::new(),
        }
    }

    pub fn padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.padding = (top, right, bottom, left);
        self
    }

    pub fn max_width_fraction(mut self, fraction: f32) -> Self {
        self.max_width_fraction = fraction;
        self
    }

    pub fn unfold(mut self) -> Self {
        self.entrance = Entrance::Unfold;
        self.overflow_visible = true;
        self
    }

    pub fn without_close(mut self) -> Self {
        self.close = false;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl RenderOnce for ModalFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let viewport = window.viewport_size();
        let width = self
            .width
            .min(f32::from(viewport.width) * self.max_width_fraction)
            .min(f32::from(viewport.width) - 36.0);
        let (top, right, bottom, left) = self.padding;
        let close = self.on_close.clone();
        let scrim_close = self.on_close.clone();
        let card_id = ElementId::Name(format!("{}-card", self.id).into());
        let entrance = self.entrance;
        let card = div()
            .id(card_id.clone())
            .relative()
            .flex()
            .flex_col()
            .w(px(width))
            .flex_none()
            .pt(px(top))
            .pr(px(right))
            .pb(px(bottom))
            .pl(px(left))
            .bg(p.surface)
            .border_1()
            .border_color(p.line_strong)
            .rounded(px(crate::theme::RADIUS))
            .when(!self.overflow_visible, |card| card.overflow_hidden())
            .shadow(p.popup_shadow())
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(self.children)
            .when(self.close, |card| {
                card.child(
                    div().absolute().top(px(14.0)).right(px(14.0)).child(
                        IconButton::new(
                            ElementId::Name(format!("{}-close", self.id).into()),
                            Key::new("modal-close", &card_id),
                            IconStyle::Close,
                        )
                        .icon(Icon::Close, 15.0)
                        .on_click(move |_, window, cx| close(window, cx)),
                    ),
                )
            });
        let (duration, curve, rise) = match entrance {
            Entrance::Pop => (Duration::from_millis(220), EASE_OUT, 12.0),
            Entrance::Unfold => (Duration::from_millis(360), [0.34, 1.42, 0.5, 1.0], 16.0),
        };
        let card = card.with_animation(
            ElementId::Name(format!("{}-pop", self.id).into()),
            Animation::new(duration),
            move |card, t| {
                let t = cubic_bezier(curve, t);
                let fade = match entrance {
                    Entrance::Pop => t,
                    Entrance::Unfold => (t / 0.55).min(1.0),
                };
                card.top(px(rise * (1.0 - t))).opacity(fade.clamp(0.0, 1.0))
            },
        );
        let fade_id = ElementId::Name(format!("{}-fade", self.id).into());
        div()
            .id(self.id)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .p(px(18.0))
            .bg(p.scrim)
            .overflow_y_scroll()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                scrim_close(window, cx)
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .w_full()
                    .min_h(px((f32::from(viewport.height) - 36.0).max(0.0)))
                    .child(div().flex_1())
                    .child(card)
                    .child(div().flex_1()),
            )
            .with_animation(
                fade_id,
                Animation::new(Duration::from_millis(180)).with_easing(crate::motion::eased(EASE)),
                |scrim, t| scrim.opacity(t),
            )
    }
}
