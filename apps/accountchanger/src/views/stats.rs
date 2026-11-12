use std::time::Duration;

use ac_api::LogStats;
use gpui::{
    Animation, AnimationExt, AnyElement, Context, ElementId, FontWeight, Hsla, IntoElement,
    ParentElement, SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::format::duration_words;
use crate::motion::EASE_OUT;
use crate::theme::{ThemeExt, rem};
use crate::ui::{Avatar, Chip, ModalFrame, modal_head};
use crate::views::account::stat_tile;
use crate::views::shell::{Overlay, Shell};

#[derive(Default)]
pub struct StatsView {
    stats: Option<Result<LogStats, ()>>,
}

enum Value {
    Plain(String, Option<Hsla>),
    Person { uuid: String, name: String },
    Missing,
}

impl StatsView {
    pub fn render(
        &mut self,
        shell: &mut Shell,
        _window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let tiles: Vec<(Value, SharedString)> = match &self.stats {
            None => Vec::new(),
            Some(Err(())) => vec![(Value::Missing, "Не удалось загрузить".into())],
            Some(Ok(stats)) => {
                let person = |uuid: &str, name: &str| {
                    if name.is_empty() {
                        Value::Missing
                    } else {
                        Value::Person {
                            uuid: uuid.to_owned(),
                            name: name.to_owned(),
                        }
                    }
                };
                vec![
                    (
                        Value::Plain(stats.total_accounts.to_string(), None),
                        "Аккаунтов в логах".into(),
                    ),
                    (
                        Value::Plain(stats.total_sessions.to_string(), None),
                        "Всего сессий запуска".into(),
                    ),
                    (
                        Value::Plain(
                            stats.active_now.to_string(),
                            (stats.active_now > 0).then_some(p.green),
                        ),
                        "Активно сейчас".into(),
                    ),
                    (
                        Value::Plain(duration_words(stats.avg_duration), None),
                        "Среднее время сессии".into(),
                    ),
                    (
                        Value::Plain(duration_words(stats.total_duration), None),
                        "Суммарное время сессий".into(),
                    ),
                    (
                        person(&stats.longest_uuid, &stats.longest_name),
                        format!(
                            "Самая долгая сессия ({})",
                            duration_words(stats.longest_duration)
                        )
                        .into(),
                    ),
                    (
                        person(&stats.most_launched_uuid, &stats.most_launched_name),
                        "Чаще всего запускался".into(),
                    ),
                    (
                        Value::Plain(stats.total_lines.to_string(), None),
                        "Строк логов записано".into(),
                    ),
                ]
            }
        };
        let store = shell.store.clone();
        let cells: Vec<AnyElement> = tiles
            .into_iter()
            .enumerate()
            .map(|(index, (value, label))| {
                let value = match value {
                    Value::Plain(text, tone) => div()
                        .text_size(rem(0.98))
                        .font(crate::theme::tabular(FontWeight::BOLD))
                        .when_some(tone, |v, tone| v.text_color(tone))
                        .child(text)
                        .into_any_element(),
                    Value::Missing => div()
                        .text_size(rem(0.98))
                        .font(crate::theme::tabular(FontWeight::BOLD))
                        .child("—")
                        .into_any_element(),
                    Value::Person { uuid, name } => {
                        let rank = {
                            let store = store.read(cx);
                            store.account(&uuid).and_then(|a| store.rank(a))
                        };
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .min_w_0()
                            .child(Avatar::new(&store, uuid, 26.0, 6.0))
                            .children(rank.map(|rank| Chip::rank(rank).compact()))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_size(rem(0.9))
                                    .font_weight(FontWeight::BOLD)
                                    .child(name),
                            )
                            .into_any_element()
                    }
                };
                let delay = Duration::from_millis(30 + 50 * index as u64);
                stat_tile(value, &label, cx)
                    .flex_1()
                    .with_animations(
                        ElementId::Name(format!("stat-tile-{index}").into()),
                        vec![
                            Animation::new(delay),
                            Animation::new(Duration::from_millis(320))
                                .with_easing(crate::motion::eased(EASE_OUT)),
                        ],
                        |tile, phase, t| {
                            let t = if phase == 0 { 0.0 } else { t };
                            tile.opacity(t)
                                .mt(px(10.0 * (1.0 - t)))
                                .mb(px(-10.0 * (1.0 - t)))
                        },
                    )
                    .into_any_element()
            })
            .collect();
        let mut rows = Vec::new();
        let mut cells = cells.into_iter();
        while let Some(first) = cells.next() {
            let second = cells.next();
            rows.push(
                div()
                    .flex()
                    .gap(px(10.0))
                    .child(div().flex_1().min_w_0().flex().child(first))
                    .child(div().flex_1().min_w_0().flex().children(second)),
            );
        }
        ModalFrame::new("stats-overlay", 460.0, Shell::closer::<StatsView>(cx))
            .padding(20.0, 20.0, 20.0, 20.0)
            .child(modal_head("Статистика", None, cx))
            .child(
                div()
                    .mt(px(14.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .children(rows),
            )
            .into_any_element()
    }
}

impl Shell {
    pub(crate) fn open_stats(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_overlay(Overlay::Stats(StatsView::default()), cx);
        let future = self.store.read(cx).backend.log_stats();
        cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |shell, cx| {
                if let Ok(stats) = &result {
                    let names = [stats.longest_name.clone(), stats.most_launched_name.clone()];
                    shell.store.update(cx, |store, cx| {
                        for name in &names {
                            store.ensure_player(name, cx);
                        }
                    });
                }
                if let Some(view) = shell.overlay::<StatsView>() {
                    view.stats = Some(result.map_err(|_| ()));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
