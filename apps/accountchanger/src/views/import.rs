use std::path::PathBuf;

use ac_api::{Credentials, ImportStatus};
use gpui::{
    AnyElement, AppContext, Context, ElementId, Entity, ExternalPaths, FontWeight,
    InteractiveElement, IntoElement, ParentElement, ScrollHandle, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder, px, relative,
};

use crate::assets::{Icon, icon};
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, alpha, mix, rem, rgba};
use crate::ui::{
    Button, ButtonStyle, InputEvent, ModalFrame, ScrollArea, TextInput, modal_head, spaced,
};
use crate::views::shell::{Overlay, Shell};
use crate::views::widgets::{drop_zone, field, text_field};

pub struct ImportView {
    list: Vec<Credentials>,
    cancelling: bool,
    editing: bool,
    scroll: ScrollHandle,
}

pub struct CredEntryView {
    index: Option<usize>,
    login: Entity<TextInput>,
    password: Entity<TextInput>,
    revealed: bool,
}

pub fn parse_credentials(text: &str) -> Vec<Credentials> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| {
            let (login, password) = line.split_once(':')?;
            let login = login.trim();
            (!login.is_empty() && !password.is_empty()).then(|| Credentials {
                login: login.to_owned(),
                password: password.to_owned(),
            })
        })
        .collect()
}

pub fn dedup(list: &mut Vec<Credentials>) -> usize {
    let before = list.len();
    let mut seen = std::collections::HashSet::new();
    list.retain(|entry| {
        let key = entry.login.trim().to_lowercase();
        !key.is_empty() && seen.insert(key)
    });
    before - list.len()
}

impl ImportView {
    pub fn render(
        &mut self,
        shell: &mut Shell,
        _window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let progress = shell.store.read(cx).import.clone();
        let showing_progress = !self.editing && (progress.running || progress.has_results());

        let count_text = if showing_progress {
            let mut text = format!("Импорт: {} сохранено", progress.ok);
            if progress.skip > 0 {
                text.push_str(&format!(" · {} уже было", progress.skip));
            }
            if progress.fail > 0 {
                text.push_str(&format!(" · {} с ошибкой", progress.fail));
            }
            text
        } else {
            format!("Подготовлено: {}", self.list.len())
        };

        let rows: Vec<AnyElement> = if showing_progress {
            progress
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let (tone, text) = match item.status {
                        ImportStatus::Ok => (p.green, "✓".to_owned()),
                        ImportStatus::Err | ImportStatus::Canceled => (p.red, item.text.clone()),
                        ImportStatus::Skip => (alpha(p.muted, 0.75), item.text.clone()),
                        ImportStatus::Pending | ImportStatus::Working => {
                            (p.blue, item.text.clone())
                        }
                    };
                    cred_row(index, &p)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .text_size(rem(0.85))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(item.login.clone()),
                        )
                        .child(
                            div()
                                .max_w(relative(0.7))
                                .text_size(rem(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(tone)
                                .text_right()
                                .child(text),
                        )
                        .into_any_element()
                })
                .collect()
        } else {
            self.list
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let dots = "•".repeat(entry.password.chars().count().min(8));
                    let remove_key = Key::new("cred-rm", index);
                    let hover = motion::value(remove_key, cx);
                    cred_row(index, &p)
                        .child(
                            div()
                                .id(ElementId::Name(format!("cred-edit-{index}").into()))
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    sound::play(Sound::Tap);
                                    this.open_cred_entry(Some(index), window, cx);
                                }))
                                .child(
                                    div()
                                        .overflow_hidden()
                                        .text_size(rem(0.85))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(entry.login.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(rem(0.7))
                                        .text_color(p.faint)
                                        .child(spaced(&dots, 1.0)),
                                ),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(format!("cred-rm-{index}").into()))
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(26.0))
                                .rounded(px(7.0))
                                .border_1()
                                .border_color(mix(p.line, alpha(p.red, 0.45), hover))
                                .text_color(mix(p.faint, p.red, hover))
                                .cursor_pointer()
                                .on_hover(move |hovered, window, cx| {
                                    motion::hover(remove_key, *hovered, window, cx)
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    sound::play(Sound::Tap);
                                    if let Some(view) = this.overlay::<ImportView>()
                                        && index < view.list.len()
                                    {
                                        view.list.remove(index);
                                    }
                                    cx.notify();
                                }))
                                .child(icon(Icon::XMark, px(11.0), mix(p.faint, p.red, hover))),
                        )
                        .into_any_element()
                })
                .collect()
        };
        let empty = rows.is_empty();
        let list = ScrollArea::new("cred-list", &self.scroll).child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .pr(px(6.0))
                .when(empty, |list| {
                    list.child(
                        div()
                            .py(px(24.0))
                            .px(px(8.0))
                            .text_center()
                            .text_size(rem(0.82))
                            .text_color(p.faint)
                            .child("Список пуст. Добавь аккаунт или перетащи .txt"),
                    )
                })
                .children(rows),
        );

        let left = div()
            .flex_none()
            .w(px(320.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(16.0))
            .border_r_1()
            .border_color(p.line)
            .child(
                Button::new("cred-add", "＋ Добавить аккаунт", ButtonStyle::Primary)
                    .full()
                    .disabled(showing_progress)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_cred_entry(None, window, cx)),
                    ),
            )
            .child(
                div()
                    .px(px(2.0))
                    .pt(px(2.0))
                    .text_size(rem(0.72))
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.faint)
                    .child(spaced(&count_text.to_uppercase(), 0.4)),
            )
            .child(list);

        let drop = drop_zone(
            "cred-drop",
            "Или перетащите .txt (login:pass построчно) / нажмите для выбора",
            false,
            cx,
        )
        .flex_1()
        .min_h(px(70.0))
        .when(showing_progress, |zone| zone.opacity(0.5))
        .when(!showing_progress, |zone| {
            zone.on_click(cx.listener(|this, _, _, cx| {
                sound::play(Sound::Tap);
                this.pick_credentials_file(cx);
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    this.ingest_credentials(path.clone(), cx);
                }
            }))
        });

        let note = div()
            .flex_none()
            .px(px(11.0))
            .py(px(9.0))
            .rounded(px(9.0))
            .border_1()
            .border_color(rgba(0xe3a400, 0.3))
            .bg(rgba(0xe3a400, 0.12))
            .text_size(rem(0.74))
            .line_height(relative(1.45))
            .text_color(p.muted)
            .child("Аккаунты обрабатываются по очереди, не мешайте импорту токенов или отмените процесс.");

        let progress_box = showing_progress.then(|| {
            let text = if progress.running {
                format!("{} / {} · вход в аккаунты…", progress.done, progress.total)
            } else {
                let mut text = format!(
                    "Готово: {} из {}",
                    progress.ok + progress.skip,
                    progress.total
                );
                if progress.fail > 0 {
                    text.push_str(&format!(" ({} с ошибкой)", progress.fail));
                }
                text
            };
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(rem(0.76))
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.muted)
                        .text_center()
                        .child(text),
                )
                .child(
                    div()
                        .h(px(8.0))
                        .rounded_full()
                        .border_1()
                        .border_color(p.line)
                        .bg(p.surface_2)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(progress.fraction()))
                                .rounded_full()
                                .bg(p.blue),
                        ),
                )
        });

        let apply_label = if self.list.is_empty() {
            "Импортировать".to_owned()
        } else {
            format!("Импортировать ({})", self.list.len())
        };
        let right = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .px(px(20.0))
            .py(px(18.0))
            .child(drop)
            .child(note)
            .children(progress_box)
            .when(!showing_progress, |col| {
                col.child(
                    Button::new("cred-apply", apply_label, ButtonStyle::Primary)
                        .full()
                        .on_click(cx.listener(|this, _, _, cx| this.start_import(cx))),
                )
            })
            .when(showing_progress && progress.running, |col| {
                col.child(
                    Button::new(
                        "cred-cancel",
                        if self.cancelling {
                            "Отменяю…"
                        } else {
                            "Отменить импорт"
                        },
                        ButtonStyle::Ghost,
                    )
                    .full()
                    .disabled(self.cancelling)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_import(cx))),
                )
            })
            .when(showing_progress && !progress.running, |col| {
                col.child(
                    Button::new("cred-newlist", "Новый импорт", ButtonStyle::Primary)
                        .full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(view) = this.overlay::<ImportView>() {
                                view.editing = true;
                                view.list.clear();
                            }
                            cx.notify();
                        })),
                )
            });

        let height = (f32::from(_window.viewport_size().height) * 0.84).min(460.0);
        ModalFrame::new("cred-overlay", 840.0, Shell::closer::<ImportView>(cx))
            .max_width_fraction(0.94)
            .child(
                div().h(px(height)).flex().flex_col().child(modal_head(
                    "Импорт аккаунтов",
                    Some("Собери список, затем нажми «Импортировать» - лаунчер войдёт в каждый и сохранит токены".into()),
                    cx,
                ))
                .child(div().flex().flex_1().min_h_0().child(left).child(right)),
            )
            .into_any_element()
    }
}

fn cred_row(index: usize, p: &crate::theme::Palette) -> gpui::Stateful<gpui::Div> {
    div()
        .id(ElementId::Name(format!("cred-row-{index}").into()))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.0))
        .px(px(11.0))
        .py(px(9.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_2)
}

impl CredEntryView {
    pub fn render(
        &mut self,
        _shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let eye_key = Key::of("credentry-eye");
        let eye_hover = motion::value(eye_key, cx);
        let password = div()
            .relative()
            .child(text_field(&self.password, window, cx).pr(px(42.0)))
            .child(
                div()
                    .absolute()
                    .right(px(6.0))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id("credentry-eye")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(30.0))
                            .rounded(px(7.0))
                            .bg(mix(rgba(0x000000, 0.0), p.surface_2, eye_hover))
                            .cursor_pointer()
                            .on_hover(move |hovered, window, cx| {
                                motion::hover(eye_key, *hovered, window, cx)
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                sound::play(Sound::Tap);
                                if let Some(view) = this.overlay::<CredEntryView>() {
                                    view.revealed = !view.revealed;
                                    let masked = !view.revealed;
                                    view.password
                                        .update(cx, |input, cx| input.set_masked(masked, cx));
                                }
                                cx.notify();
                            }))
                            .child(icon(
                                if self.revealed {
                                    Icon::EyeOff
                                } else {
                                    Icon::Eye
                                },
                                px(18.0),
                                if self.revealed {
                                    p.blue
                                } else {
                                    mix(p.muted, p.text, eye_hover)
                                },
                            )),
                    ),
            );
        ModalFrame::new(
            "credentry-overlay",
            420.0,
            Shell::closer::<CredEntryView>(cx),
        )
        .child(modal_head(
            if self.index.is_some() {
                "Изменить аккаунт"
            } else {
                "Новый аккаунт"
            },
            None,
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .pt(px(18.0))
                .px(px(20.0))
                .pb(px(20.0))
                .child(field("Логин", text_field(&self.login, window, cx), cx))
                .child(field("Пароль", password, cx))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(10.0))
                        .mt(px(4.0))
                        .child(
                            Button::new("credentry-cancel", "Отмена", ButtonStyle::Ghost).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.close::<CredEntryView>(window, cx)
                                }),
                            ),
                        )
                        .child(
                            Button::new("credentry-save", "Добавить", ButtonStyle::Primary)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save_cred_entry(window, cx)
                                })),
                        ),
                ),
        )
        .into_any_element()
    }
}

impl Shell {
    pub(crate) fn open_import(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let progress = self.store.read(cx).import.clone();
        let editing = !(progress.running || progress.has_results());
        self.push_overlay(
            Overlay::Import(ImportView {
                list: Vec::new(),
                cancelling: false,
                editing,
                scroll: ScrollHandle::new(),
            }),
            cx,
        );
    }

    fn open_cred_entry(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = index.and_then(|index| {
            self.overlay::<ImportView>()
                .and_then(|view| view.list.get(index).cloned())
        });
        let login = cx.new(|cx| {
            let mut input = TextInput::new("Логин", window, cx);
            if let Some(entry) = &existing {
                input.set_text(entry.login.clone(), cx);
            }
            input
        });
        let password = cx.new(|cx| {
            let mut input = TextInput::new("Пароль", window, cx);
            input.set_masked(true, cx);
            if let Some(entry) = &existing {
                input.set_text(entry.password.clone(), cx);
            }
            input
        });
        cx.subscribe_in(&password, window, |this, _, event, window, cx| {
            if *event == InputEvent::Submit {
                this.save_cred_entry(window, cx);
            }
        })
        .detach();
        let focus = password.clone();
        cx.subscribe_in(&login, window, move |_, _, event, window, cx| {
            if *event == InputEvent::Submit {
                focus.read(cx).focus(window);
            }
        })
        .detach();
        login.read(cx).focus(window);
        self.push_overlay(
            Overlay::CredEntry(CredEntryView {
                index,
                login,
                password,
                revealed: false,
            }),
            cx,
        );
    }

    fn save_cred_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.overlay::<CredEntryView>() else {
            return;
        };
        let login = entry.login.read(cx).text().trim().to_owned();
        let password = entry.password.read(cx).text().to_owned();
        let index = entry.index;
        if login.is_empty() || password.is_empty() {
            self.toast("Укажи логин и пароль", true, cx);
            return;
        }
        if let Some(view) = self.overlay::<ImportView>() {
            let credentials = Credentials { login, password };
            match index.filter(|index| *index < view.list.len()) {
                Some(index) => view.list[index] = credentials,
                None => view.list.push(credentials),
            }
            dedup(&mut view.list);
        }
        self.close::<CredEntryView>(window, cx);
    }

    fn pick_credentials_file(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("Текст", &["txt"])
                .pick_file()
                .await
            else {
                return;
            };
            let path = file.path().to_path_buf();
            this.update(cx, |shell, cx| shell.ingest_credentials(path, cx))
                .ok();
        })
        .detach();
    }

    fn ingest_credentials(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let parsed = parse_credentials(&text);
        if parsed.is_empty() {
            self.toast("В файле нет строк формата login:pass", true, cx);
            return;
        }
        let added = parsed.len();
        let Some(view) = self.overlay::<ImportView>() else {
            return;
        };
        view.list.extend(parsed);
        let removed = dedup(&mut view.list);
        let mut message = format!("Добавлено: {}", added.saturating_sub(removed));
        if removed > 0 {
            message.push_str(&format!(" (дублей убрано: {removed})"));
        }
        self.toast(message, false, cx);
        cx.notify();
    }

    fn start_import(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<ImportView>() else {
            return;
        };
        if view.list.is_empty() {
            self.toast("Добавь хотя бы один аккаунт", true, cx);
            return;
        }
        let accounts = view.list.clone();
        view.cancelling = false;
        let future = self.store.read(cx).backend.import_start(accounts);
        cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |shell, cx| match result {
                Ok(()) => {
                    if let Some(view) = shell.overlay::<ImportView>() {
                        view.editing = false;
                    }
                    shell.toast(
                        "Импорт запущен - можно закрыть окно, продолжит в фоне",
                        false,
                        cx,
                    );
                }
                Err(error) => shell.toast(error.message().to_owned(), true, cx),
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn finish_cancelled_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let running = self.store.read(cx).import.running;
        let cancelled = self
            .overlay::<ImportView>()
            .is_some_and(|view| view.cancelling);
        if cancelled && !running {
            self.close::<ImportView>(window, cx);
        }
    }

    fn cancel_import(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<ImportView>() else {
            return;
        };
        if view.cancelling {
            return;
        }
        view.cancelling = true;
        self.toast("Отмена импорта…", false, cx);
        self.store.update(cx, |store, cx| {
            let future = store.backend.import_cancel();
            store.act(cx, future, "");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_login_password_lines() {
        let list = parse_credentials("alpha:pass\n\nbad\n:nologin\nbeta: spaced:colon\r\n");
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].login, "beta");
        assert_eq!(list[1].password, " spaced:colon");
    }

    #[test]
    fn dedup_is_case_insensitive() {
        let mut list = parse_credentials("A:1\na:2\nb:3");
        assert_eq!(dedup(&mut list), 1);
        assert_eq!(list.len(), 2);
    }
}
