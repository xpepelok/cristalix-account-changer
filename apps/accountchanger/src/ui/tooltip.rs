use gpui::{
    AnimationExt, App, Bounds, FontWeight, Global, InteractiveElement, IntoElement, ParentElement,
    Pixels, SharedString, Stateful, StatefulInteractiveElement, Styled, TextRun, Window, canvas,
    div, prelude::FluentBuilder, px,
};

use crate::motion::Key;
use crate::theme::{ThemeExt, rem, rgba, shadow};

const GAP: f32 = 8.0;
const EDGE: f32 = 6.0;
const MAX_WIDTH: f32 = 280.0;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 6.0;
const LINE: f32 = 1.35;

#[derive(Default)]
pub struct Tooltips {
    hovered: Option<(Key, SharedString)>,
    bounds: Option<Bounds<Pixels>>,
    suppressed: Option<Key>,
}

impl Global for Tooltips {}

impl Tooltips {
    pub fn dismiss(cx: &mut App) {
        let state = cx.default_global::<Tooltips>();
        if let Some((key, _)) = state.hovered.take() {
            state.suppressed = Some(key);
        }
        state.bounds = None;
    }
}

pub fn with_tooltip(
    element: Stateful<gpui::Div>,
    key: Key,
    text: SharedString,
) -> Stateful<gpui::Div> {
    let tracked = text.clone();
    element
        .on_any_mouse_down(|_, window, cx| {
            Tooltips::dismiss(cx);
            window.refresh();
        })
        .child(
            div()
                .id("tooltip-anchor")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .on_hover(move |hovered, window, cx| {
                    let state = cx.default_global::<Tooltips>();
                    if *hovered {
                        if state.suppressed != Some(key) {
                            state.hovered = Some((key, tracked.clone()));
                        }
                    } else {
                        if state.hovered.as_ref().is_some_and(|(k, _)| *k == key) {
                            state.hovered = None;
                            state.bounds = None;
                        }
                        if state.suppressed == Some(key) {
                            state.suppressed = None;
                        }
                    }
                    window.refresh();
                })
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            let state = cx.default_global::<Tooltips>();
                            if let Some((current, label)) = &mut state.hovered
                                && *current == key
                            {
                                *label = text.clone();
                                state.bounds = Some(bounds);
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .size_full(),
                ),
        )
}

pub fn tooltip_overlay(window: &mut Window, cx: &mut App) -> Option<impl IntoElement + use<>> {
    let state = cx.try_global::<Tooltips>()?;
    let (key, text) = state.hovered.clone()?;
    let anchor = state.bounds?;
    let p = cx.palette();
    let font_size = rem(0.76);
    let run = TextRun {
        len: text.len(),
        font: gpui::Font {
            weight: FontWeight::MEDIUM,
            ..gpui::font(crate::theme::FONT_FAMILY)
        },
        color: p.text,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let text_width = f32::from(
        window
            .text_system()
            .shape_line(text.clone(), font_size, &[run], None)
            .width,
    );
    let viewport = window.viewport_size();
    let vw = f32::from(viewport.width);
    let natural = text_width.ceil() + PAD_X * 2.0 + 4.0;
    let width = natural.min(MAX_WIDTH);
    let wraps = natural > MAX_WIDTH;
    let height = f32::from(font_size) * LINE * if wraps { 2.0 } else { 1.0 } + PAD_Y * 2.0 + 2.0;
    let center = f32::from(anchor.origin.x) + f32::from(anchor.size.width) / 2.0;
    let left = (center - width / 2.0).max(EDGE).min(vw - width - EDGE);
    let above = f32::from(anchor.origin.y) - GAP - height;
    let top = if above < EDGE {
        f32::from(anchor.origin.y + anchor.size.height) + GAP
    } else {
        above
    };
    Some(
        div()
            .absolute()
            .top(px(top))
            .left(px(left))
            .w(px(width))
            .px(px(PAD_X))
            .py(px(PAD_Y))
            .rounded(px(8.0))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.surface_2)
            .shadow(shadow(0.0, 8.0, 22.0, 0.0, rgba(0x000000, 0.5)))
            .text_size(font_size)
            .font_weight(FontWeight::MEDIUM)
            .line_height(gpui::relative(LINE))
            .text_color(p.text)
            .when(!wraps, |tip| tip.whitespace_nowrap())
            .child(text)
            .with_animation(
                gpui::ElementId::Name(format!("tooltip-{key:?}").into()),
                gpui::Animation::new(std::time::Duration::from_millis(120))
                    .with_easing(crate::motion::eased(crate::motion::EASE)),
                |tip, t| tip.opacity(t).mt(px(3.0 * (1.0 - t))),
            ),
    )
}
