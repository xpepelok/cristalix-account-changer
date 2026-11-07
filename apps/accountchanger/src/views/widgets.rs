use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Context, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, Radians,
    SharedString, StatefulInteractiveElement, Styled, Transformation, Window, deferred, div,
    prelude::FluentBuilder, px,
};

use crate::assets::{Icon, icon};
use crate::motion::{self, EASE, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, mix, rem, rgba, shadow};
use crate::ui::{drop_in, menu_surface, spaced};
use crate::views::shell::{Popup, Shell};

pub type ChangeHandler = Rc<dyn Fn(&mut Shell, String, &mut Window, &mut Context<Shell>)>;

pub fn field_label(text: &str, cx: &gpui::App) -> impl IntoElement + use<> {
    let p = cx.palette();
    div()
        .flex_none()
        .text_size(rem(0.62))
        .font_weight(FontWeight::BOLD)
        .text_color(p.faint)
        .child(spaced(&text.to_uppercase(), 0.4))
}

pub fn text_field(
    input: &gpui::Entity<crate::ui::TextInput>,
    window: &Window,
    cx: &gpui::App,
) -> gpui::Div {
    let p = cx.palette();
    let focused = input.read(cx).is_focused(window);
    div()
        .flex()
        .items_center()
        .w_full()
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(if focused { p.blue } else { p.line })
        .bg(if focused { p.surface } else { p.surface_2 })
        .text_size(rem(0.88))
        .child(input.clone())
}

pub fn field(label: &'static str, body: impl IntoElement, cx: &gpui::App) -> gpui::Div {
    let p = cx.palette();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(rem(0.72))
                .font_weight(FontWeight::BOLD)
                .text_color(p.faint)
                .child(spaced(&label.to_uppercase(), 0.4)),
        )
        .child(body)
}

pub fn drop_zone(
    id: &'static str,
    hint: &'static str,
    active: bool,
    cx: &gpui::App,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.palette();
    let key = Key::of(id);
    let hover = if active { 1.0 } else { motion::value(key, cx) };
    div()
        .id(id)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .px(px(20.0))
        .py(px(30.0))
        .rounded(px(14.0))
        .border_2()
        .border_dashed()
        .border_color(mix(p.line_strong, p.blue, hover))
        .bg(mix(p.surface_2, rgba(0x3b82e8, 0.10), hover))
        .text_color(mix(p.muted, p.text, hover))
        .cursor_pointer()
        .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(44.0))
                .rounded_full()
                .bg(rgba(0x3b82e8, 0.16))
                .child(icon(Icon::Plus, px(24.0), p.blue)),
        )
        .child(
            div()
                .max_w(px(260.0))
                .text_size(rem(0.82))
                .line_height(gpui::relative(1.4))
                .text_center()
                .child(hint),
        )
}

impl Shell {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_dropdown(
        &mut self,
        id: &'static str,
        options: Vec<(String, String)>,
        current: String,
        borderless: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_change: impl Fn(&mut Shell, String, &mut Window, &mut Context<Shell>) + 'static,
    ) -> gpui::AnyElement {
        let p = cx.palette();
        let shown = self.popup_shown(&Popup::Dropdown(id));
        let open = shown == Some(None);
        let closing = shown.flatten();
        let key = Key::new("dropdown", id);
        let caret_key = Key::new("dropdown-caret", id);
        if motion::is_on(caret_key, cx) != open {
            motion::drive(
                caret_key,
                open,
                Duration::from_millis(160),
                EASE,
                window,
                cx,
            );
        }
        let turn = motion::value(caret_key, cx);
        let hover = motion::value(key, cx);
        let label = options
            .iter()
            .find(|(value, _)| *value == current)
            .or(options.first())
            .map(|(_, label)| label.clone())
            .unwrap_or_default();
        let border = if borderless {
            rgba(0x000000, 0.0)
        } else if open {
            p.blue
        } else {
            mix(p.line, p.line_strong, hover)
        };
        let trigger =
            div()
                .id(ElementId::Name(format!("{id}-trigger").into()))
                .flex()
                .items_center()
                .gap(px(8.0))
                .w_full()
                .rounded(px(9.0))
                .border_1()
                .border_color(border)
                .when(!borderless, |t| t.bg(p.surface_2).px(px(11.0)).py(px(8.0)))
                .when(borderless, |t| t.px(px(8.0)).py(px(7.0)))
                .text_size(rem(0.8))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text)
                .cursor_pointer()
                .occlude()
                .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
                .on_click(cx.listener(move |this, _, _, cx| {
                    sound::play(Sound::Tap);
                    this.toggle_popup(Popup::Dropdown(id), cx);
                }))
                .child(div().flex_1().min_w_0().overflow_hidden().child(label))
                .child(icon(Icon::Caret, px(15.0), p.muted).with_transformation(
                    Transformation::rotate(Radians(std::f32::consts::PI * turn)),
                ));
        let handler: ChangeHandler = Rc::new(on_change);
        div()
            .relative()
            .w_full()
            .child(trigger)
            .when(shown.is_some(), |dropdown| {
                let items = options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (value, label))| {
                        let active = value == current;
                        let handler = handler.clone();
                        div()
                            .id(index)
                            .flex_none()
                            .px(px(11.0))
                            .py(px(9.0))
                            .rounded(px(7.0))
                            .overflow_hidden()
                            .text_size(rem(0.82))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .cursor_pointer()
                            .when(active, |item| item.bg(rgba(0x3b82e8, 0.14)))
                            .when(!active, |item| item.hover(|s| s.bg(p.surface_2)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                sound::play(Sound::Tap);
                                this.close_popup(cx);
                                handler(this, value.clone(), window, cx);
                            }))
                            .child(SharedString::from(label))
                    });
                let menu = menu_surface(p.surface, 10.0, 6.0, cx)
                    .gap(px(3.0))
                    .max_h(px(220.0))
                    .shadow(shadow(0.0, 12.0, 30.0, 0.0, rgba(0x000000, p.popup_shadow)))
                    .children(items);
                dropdown.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(if borderless { 38.0 } else { 41.0 }))
                            .left_0()
                            .right_0()
                            .child(drop_in(
                                ElementId::Name(format!("{id}-menu-in").into()),
                                menu.id(ElementId::Name(format!("{id}-menu").into()))
                                    .overflow_y_scroll()
                                    .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                                        this.dismiss_popup(Popup::Dropdown(id), cx)
                                    })),
                                closing,
                            )),
                    )
                    .with_priority(2),
                )
            })
            .into_any_element()
    }
}
