use ac_api::OnlineStats;
use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, deferred, div, prelude::FluentBuilder, px, relative,
};

use crate::assets::{Icon, icon};
use crate::format::bytes;
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, mix, rem};
use crate::ui::{drop_in, menu_surface, pop_in, with_tooltip};
use crate::views::shell::{Popup, Shell};

impl Shell {
    pub(crate) fn render_notifications(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let update = store.update.clone().filter(|info| info.available);
        let progress = store.update_progress.clone();
        let mut panel = menu_surface(p.surface, 12.0, 10.0, cx).w(px(320.0));
        panel = match update {
            None => panel.child(
                div()
                    .py(px(26.0))
                    .px(px(12.0))
                    .flex()
                    .justify_center()
                    .text_size(rem(0.82))
                    .text_color(p.faint)
                    .child("Новых уведомлений нет"),
            ),
            Some(info) => {
                let notes = if info.notes.trim().is_empty() {
                    "Новая версия готова к установке.".to_owned()
                } else {
                    info.notes.trim().to_owned()
                };
                let busy = progress.as_ref().is_some_and(|p| p.active || p.done);
                let key = Key::of("update-download");
                let hover = if busy { 0.0 } else { motion::value(key, cx) };
                let bar = progress.filter(|_| self.update_started).map(|progress| {
                    let (fill, text) = if !progress.error.is_empty() {
                        (0.0, "Ошибка загрузки".to_owned())
                    } else if progress.done {
                        (1.0, "Установлено, перезапуск…".to_owned())
                    } else if progress.total > 0 {
                        (
                            progress.percent() as f32 / 100.0,
                            format!(
                                "Скачивание {}% ({} / {})",
                                progress.percent(),
                                bytes(progress.downloaded),
                                bytes(progress.total)
                            ),
                        )
                    } else if progress.active {
                        (0.0, format!("Скачивание {}…", bytes(progress.downloaded)))
                    } else {
                        (0.0, "Подготовка…".to_owned())
                    };
                    div()
                        .relative()
                        .mt(px(12.0))
                        .mr(px(42.0))
                        .h(px(20.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.surface)
                        .overflow_hidden()
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .w(relative(fill))
                                .bg(p.blue),
                        )
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(rem(0.66))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(text),
                        )
                });
                panel.child(
                    div()
                        .relative()
                        .pt(px(13.0))
                        .px(px(14.0))
                        .pb(px(54.0))
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.surface_2)
                        .child(
                            div()
                                .pr(px(30.0))
                                .text_size(rem(0.86))
                                .font_weight(FontWeight::BOLD)
                                .child(format!("Доступно обновление {}", info.latest)),
                        )
                        .child(
                            div()
                                .id("notif-desc")
                                .mt(px(5.0))
                                .max_h(px(120.0))
                                .overflow_y_scroll()
                                .text_size(rem(0.76))
                                .line_height(relative(1.45))
                                .text_color(p.muted)
                                .child(format!("Текущая версия: {}\n\n{}", info.current, notes)),
                        )
                        .children(bar)
                        .child(
                            div()
                                .absolute()
                                .right(px(10.0))
                                .bottom(px(10.0))
                                .child(with_tooltip(
                                    div()
                                        .id("update-download")
                                        .relative()
                                        .top(px(-hover))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(34.0))
                                        .rounded(px(9.0))
                                        .bg(mix(p.blue, p.blue_hover, hover))
                                        .when(busy, |b| b.opacity(0.6))
                                        .when(!busy, |b| {
                                            b.cursor_pointer()
                                                .on_hover(move |hovered, window, cx| {
                                                    motion::hover(key, *hovered, window, cx)
                                                })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    sound::play(Sound::Tap);
                                                    this.apply_update(cx);
                                                }))
                                        })
                                        .child(icon(Icon::Download, px(18.0), p.white)),
                                    key,
                                    "Скачать и установить".into(),
                                )),
                        ),
                )
            }
        };
        deferred(div().absolute().top(px(44.0)).right_0().child(drop_in(
            "notif-panel-in",
            panel.id("notif-panel").on_mouse_down_out(
                cx.listener(|this, _, _, cx| this.dismiss_popup(Popup::Bell, cx)),
            ),
            self.popup_shown(&Popup::Bell).flatten(),
        )))
        .with_priority(1)
    }

    fn apply_update(&mut self, cx: &mut Context<Self>) {
        self.update_started = true;
        self.store.update(cx, |store, cx| {
            store.update_progress = Some(ac_api::UpdateProgress::default());
            let future = store.backend.apply_update();
            store.fetch(cx, future, |_, (), cx| cx.notify());
        });
        cx.notify();
    }

    pub(crate) fn render_online_pop(
        &mut self,
        stats: OnlineStats,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let body = match stats {
            OnlineStats::Live(counts) => {
                let row = |label: &'static str, value: u64, first: bool| {
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(16.0))
                        .py(px(5.0))
                        .when(!first, |r| r.border_t_1().border_color(p.line))
                        .text_size(rem(0.82))
                        .text_color(p.muted)
                        .child(label)
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if first { p.online } else { p.text })
                                .child(value.to_string()),
                        )
                };
                div()
                    .child(row("Сейчас онлайн", counts.online, true))
                    .child(row("Активных за 7 дней", counts.active, false))
                    .child(row("Всего пользователей", counts.total, false))
            }
            OnlineStats::Disabled => note(
                "Вы отключили отправку метрик - онлайн недоступен. Включить можно в настройках.",
                cx,
            ),
            OnlineStats::Unavailable => note("Онлайн временно недоступен.", cx),
        };
        deferred(
            div().absolute().top(px(34.0)).left_0().child(pop_in(
                "online-pop-in",
                menu_surface(p.surface_2, 11.0, 0.0, cx)
                    .px(px(12.0))
                    .py(px(8.0))
                    .w(px(210.0))
                    .child(body)
                    .id("online-pop")
                    .on_mouse_down_out(
                        cx.listener(|this, _, _, cx| this.dismiss_popup(Popup::Online, cx)),
                    ),
            )),
        )
        .with_priority(1)
    }
}

fn note(text: &'static str, cx: &gpui::App) -> gpui::Div {
    let p = cx.palette();
    div()
        .max_w(px(200.0))
        .text_size(rem(0.78))
        .line_height(relative(1.4))
        .text_color(p.muted)
        .child(text)
}
