use gpui::{
    AnyElement, AppContext, Context, Entity, FontWeight, IntoElement, ParentElement, Styled,
    Window, div, px,
};

use crate::assets::{Icon, icon};
use crate::theme::{ThemeExt, alpha, rem};
use crate::ui::{Button, ButtonStyle, InputEvent, ModalFrame, TextInput};
use crate::views::shell::{Overlay, Shell};

type PromptHandler = Box<dyn FnOnce(&mut Shell, Option<String>, &mut Window, &mut Context<Shell>)>;
type ConfirmHandler = Box<dyn FnOnce(&mut Shell, &mut Window, &mut Context<Shell>)>;

pub struct PromptView {
    title: String,
    input: Entity<TextInput>,
    on_done: Option<PromptHandler>,
}

impl PromptView {
    pub fn resolve(
        &mut self,
        value: Option<String>,
        shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) {
        if let Some(on_done) = self.on_done.take() {
            on_done(shell, value, window, cx);
        }
    }

    pub fn render(
        &mut self,
        _shell: &mut Shell,
        _window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let focused = self.input.read(cx).is_focused(_window);
        ModalFrame::new("prompt-overlay", 380.0, Shell::closer::<PromptView>(cx))
            .without_close()
            .padding(20.0, 20.0, 20.0, 20.0)
            .child(
                div()
                    .mb(px(12.0))
                    .text_size(rem(1.0))
                    .font_weight(FontWeight::BOLD)
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(px(11.0))
                    .py(px(8.0))
                    .rounded(px(9.0))
                    .border_1()
                    .border_color(if focused { p.blue } else { p.line })
                    .bg(p.surface_2)
                    .text_size(rem(0.8))
                    .child(self.input.clone()),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(16.0))
                    .child(
                        Button::new("prompt-cancel", "Отмена", ButtonStyle::Ghost).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.finish_prompt(false, window, cx)
                            }),
                        ),
                    )
                    .child(
                        Button::new("prompt-ok", "Сохранить", ButtonStyle::Primary).on_click(
                            cx.listener(|this, _, window, cx| this.finish_prompt(true, window, cx)),
                        ),
                    ),
            )
            .into_any_element()
    }
}

pub struct ConfirmView {
    title: String,
    text: String,
    yes: String,
    on_yes: Option<ConfirmHandler>,
}

impl ConfirmView {
    pub fn resolve(
        &mut self,
        accepted: bool,
        shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) {
        if accepted && let Some(on_yes) = self.on_yes.take() {
            on_yes(shell, window, cx);
        }
    }

    pub fn render(
        &mut self,
        _shell: &mut Shell,
        _window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        ModalFrame::new("confirm-overlay", 400.0, Shell::closer::<ConfirmView>(cx))
            .without_close()
            .padding(26.0, 24.0, 20.0, 24.0)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(54.0))
                            .mb(px(14.0))
                            .rounded(px(14.0))
                            .bg(alpha(p.red, 0.14))
                            .child(icon(Icon::Trash, px(26.0), p.red)),
                    )
                    .child(
                        div()
                            .text_size(rem(1.1))
                            .font_weight(FontWeight::BOLD)
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .mt(px(7.0))
                            .text_size(rem(0.86))
                            .line_height(gpui::relative(1.5))
                            .text_color(p.muted)
                            .text_center()
                            .child(self.text.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .w_full()
                            .gap(px(8.0))
                            .mt(px(20.0))
                            .child(
                                Button::new("confirm-yes", self.yes.clone(), ButtonStyle::Danger)
                                    .grow()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.finish_confirm(true, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("confirm-no", "Нет", ButtonStyle::Ghost)
                                    .grow()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.finish_confirm(false, window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Shell {
    pub(crate) fn prompt(
        &mut self,
        title: impl Into<String>,
        initial: impl Into<String>,
        max_chars: usize,
        on_done: impl FnOnce(&mut Shell, String, &mut Window, &mut Context<Shell>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let initial = initial.into();
        let input = cx.new(|cx| {
            let mut input = TextInput::new("например: Анархия", window, cx).max_chars(max_chars);
            input.set_text(initial, cx);
            input.select_all_text(cx);
            input
        });
        cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if *event == InputEvent::Submit {
                this.finish_prompt(true, window, cx);
            }
        })
        .detach();
        input.read(cx).focus(window);
        let on_done: PromptHandler = Box::new(move |shell, value, window, cx| {
            if let Some(value) = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) {
                on_done(shell, value, window, cx);
            }
        });
        self.push_overlay(
            Overlay::Prompt(PromptView {
                title: title.into(),
                input,
                on_done: Some(on_done),
            }),
            cx,
        );
    }

    fn finish_prompt(&mut self, accept: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self
            .overlays
            .iter()
            .rposition(|overlay| matches!(overlay, Overlay::Prompt(_)))
        else {
            return;
        };
        let Overlay::Prompt(mut prompt) = self.overlays.remove(index) else {
            return;
        };
        let value = accept.then(|| prompt.input.read(cx).text().to_owned());
        let handler = prompt.on_done.take();
        self.retire(Overlay::Prompt(prompt), window, cx);
        if let Some(handler) = handler {
            handler(self, value, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn confirm(
        &mut self,
        title: impl Into<String>,
        text: impl Into<String>,
        yes: impl Into<String>,
        on_yes: impl FnOnce(&mut Shell, &mut Window, &mut Context<Shell>) + 'static,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.push_overlay(
            Overlay::Confirm(ConfirmView {
                title: title.into(),
                text: text.into(),
                yes: yes.into(),
                on_yes: Some(Box::new(on_yes)),
            }),
            cx,
        );
    }

    fn finish_confirm(&mut self, accept: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self
            .overlays
            .iter()
            .rposition(|overlay| matches!(overlay, Overlay::Confirm(_)))
        else {
            return;
        };
        let Overlay::Confirm(mut confirm) = self.overlays.remove(index) else {
            return;
        };
        let handler = confirm.on_yes.take().filter(|_| accept);
        self.retire(Overlay::Confirm(confirm), window, cx);
        if let Some(handler) = handler {
            handler(self, window, cx);
        }
        cx.notify();
    }
}
