use std::sync::Arc;
use std::time::Duration;

use ac_api::{LogLine, LogSession, LogSummary, LogView};
use gpui::{
    AnyElement, Context, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    ListAlignment, ListState, MouseButton, ParentElement, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, deferred, div, list, prelude::FluentBuilder,
    px, relative,
};

use crate::format::{
    clock_duration, count, megabytes, now_secs, session_day, session_label, session_time,
};
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, alpha, mix, rem, rgba};
use crate::ui::{
    Avatar, Chip, IconButton, IconStyle, MenuItem, ModalFrame, ScrollArea, drop_in, menu_item,
    menu_surface, modal_head,
};
use crate::views::shell::{Overlay, Popup, Shell};

const LOG_FONT: &str = "Consolas";

pub struct LogsView {
    summaries: Vec<LogSummary>,
    loaded: bool,
    uuid: Option<String>,
    session: Option<String>,
    log: Arc<Vec<LogLine>>,
    sessions: Vec<LogSession>,
    labels: Vec<SharedString>,
    lines: ListState,
    side_scroll: ScrollHandle,
    sessions_scroll: ScrollHandle,
    ticker: Option<Task<()>>,
    selection: Option<(usize, usize)>,
    selecting: bool,
}

fn session_labels(sessions: &[LogSession]) -> Vec<SharedString> {
    let days: Vec<_> = sessions
        .iter()
        .map(|session| session_day(session.started))
        .collect();
    sessions
        .iter()
        .zip(&days)
        .map(|(session, day)| {
            if session.active {
                return "Текущая".into();
            }
            let ordinal = sessions
                .iter()
                .zip(&days)
                .filter(|(other, other_day)| *other_day == day && other.started <= session.started)
                .count();
            session_label(session.started, ordinal).into()
        })
        .collect()
}

impl LogsView {
    fn new() -> Self {
        Self {
            summaries: Vec::new(),
            loaded: false,
            uuid: None,
            session: None,
            log: Arc::default(),
            sessions: Vec::new(),
            labels: Vec::new(),
            lines: ListState::new(0, ListAlignment::Bottom, px(400.0)),
            side_scroll: ScrollHandle::new(),
            sessions_scroll: ScrollHandle::new(),
            ticker: None,
            selection: None,
            selecting: false,
        }
    }

    fn current(&self) -> Option<&LogSession> {
        let id = self.session.as_ref()?;
        self.sessions.iter().find(|session| &session.id == id)
    }

    fn set_log(&mut self, log: LogView) {
        self.labels = session_labels(&log.sessions);
        self.sessions = log.sessions;
        self.log = Arc::new(log.lines);
    }

    fn clear_log(&mut self) {
        self.log = Arc::default();
        self.sessions = Vec::new();
        self.labels = Vec::new();
    }

    pub fn render(
        &mut self,
        shell: &mut Shell,
        _window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let store = shell.store.clone();
        let selected = self.uuid.clone();
        let items: Vec<AnyElement> = self
            .summaries
            .iter()
            .map(|summary| {
                let active = selected.as_deref() == Some(summary.uuid.as_str());
                let rank = {
                    let store = store.read(cx);
                    store.account(&summary.uuid).and_then(|a| store.rank(a))
                };
                let name = if summary.name.is_empty() {
                    summary.uuid.chars().take(8).collect()
                } else {
                    summary.name.clone()
                };
                let key = Key::new("logs-item", &summary.uuid);
                let hover = motion::value(key, cx);
                let uuid = summary.uuid.clone();
                div()
                    .id(ElementId::Name(format!("logs-{}", summary.uuid).into()))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(9.0))
                    .py(px(8.0))
                    .rounded(px(9.0))
                    .border_1()
                    .border_color(if active {
                        p.blue
                    } else {
                        mix(p.line, p.line_strong, hover)
                    })
                    .bg(if active {
                        rgba(0x3b82e8, 0.1)
                    } else {
                        p.surface_2
                    })
                    .cursor_pointer()
                    .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        sound::play(Sound::Tap);
                        this.select_log(uuid.clone(), cx);
                    }))
                    .child(
                        div()
                            .flex_none()
                            .size(px(8.0))
                            .rounded_full()
                            .bg(if summary.active { p.green } else { p.faint })
                            .when(summary.active, |dot| {
                                dot.shadow(crate::theme::shadow(
                                    0.0,
                                    0.0,
                                    7.0,
                                    0.0,
                                    rgba(0x1fa344, 0.8),
                                ))
                            }),
                    )
                    .child(Avatar::new(&store, summary.uuid.clone(), 22.0, 5.0))
                    .children(rank.map(|rank| Chip::rank(rank).compact()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_size(rem(0.82))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_none()
                            .ml_auto()
                            .pl(px(8.0))
                            .text_size(rem(0.68))
                            .text_color(p.muted)
                            .child(count(summary.lines)),
                    )
                    .into_any_element()
            })
            .collect();
        let side_body = if self.summaries.is_empty() && self.loaded {
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .px(px(8.0))
                .py(px(16.0))
                .text_size(rem(0.8))
                .text_color(p.muted)
                .text_center()
                .child("Пока никого не запускали через ченджер")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .children(items)
                .into_any_element()
        };
        let side = div()
            .flex_none()
            .w(px(250.0))
            .flex()
            .flex_col()
            .pr(px(14.0))
            .border_r_1()
            .border_color(p.line)
            .child(ScrollArea::new("logs-side", &self.side_scroll).child(side_body));

        let title = match &self.uuid {
            Some(uuid) => store
                .read(cx)
                .account(uuid)
                .map(|a| a.display_name().to_owned())
                .unwrap_or_else(|| uuid.chars().take(8).collect()),
            None => "Выбери аккаунт слева".to_owned(),
        };
        let current = self.current().cloned();
        let meta = current.as_ref().map(|session| {
            let pending = session.active;
            let end = if pending {
                "Активна".to_owned()
            } else {
                session_time(session.ended)
            };
            let finish = if pending {
                now_secs()
            } else {
                session.ended.max(session.started)
            };
            let tile = |dot: Hsla, pulse: bool, value: String, label: &'static str, green: bool| {
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .px(px(12.0))
                    .py(px(9.0))
                    .rounded(px(10.0))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.surface_2)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.0))
                            .min_w_0()
                            .text_size(rem(0.86))
                            .font(crate::theme::tabular(FontWeight::BOLD))
                            .text_color(if green { p.green } else { p.text })
                            .child(
                                div()
                                    .flex_none()
                                    .size(px(7.0))
                                    .rounded_full()
                                    .bg(dot)
                                    .shadow(crate::theme::shadow(
                                        0.0,
                                        0.0,
                                        0.0,
                                        3.0,
                                        alpha(dot, 0.2),
                                    ))
                                    .when(pulse, |dot| {
                                        let phase =
                                            crate::ambient::cycle(Duration::from_millis(1400), cx)
                                                .unwrap_or(0.0);
                                        dot.opacity(
                                            1.0 - 0.65 * (phase * std::f32::consts::PI).sin(),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(value),
                            ),
                    )
                    .child(
                        div()
                            .text_size(rem(0.62))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.faint)
                            .child(crate::ui::spaced(&label.to_uppercase(), 0.4)),
                    )
            };
            div()
                .flex()
                .gap(px(8.0))
                .mb(px(10.0))
                .child(tile(
                    p.green,
                    false,
                    session_time(session.started),
                    "Начало сессии",
                    false,
                ))
                .child(tile(
                    p.blue,
                    false,
                    if session.started == 0 {
                        "—".to_owned()
                    } else {
                        clock_duration(finish - session.started)
                    },
                    "Длительность сессии",
                    pending,
                ))
                .child(tile(
                    if pending { p.green } else { p.red },
                    pending,
                    end,
                    "Конец сессии",
                    pending,
                ))
        });

        let view_bg = p.logs_bg;
        let empty_text = if self.uuid.is_none() {
            Some("Выбери аккаунт слева, чтобы увидеть его лог")
        } else if self.log.is_empty() {
            Some("Лог пуст")
        } else {
            None
        };
        let lines = self.log.clone();
        let text_color = p.logs_text;
        let selection = self.selection;
        let selection_bg = alpha(p.blue, 0.3);
        let shell = cx.entity();
        let log = div()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    if let Some(view) = this.overlay::<LogsView>() {
                        view.selecting = false;
                    }
                }),
            )
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .rounded(px(10.0))
            .border_1()
            .border_color(p.line)
            .bg(view_bg)
            .overflow_hidden()
            .map(|view| match empty_text {
                Some(text) => view
                    .items_center()
                    .justify_center()
                    .text_size(rem(0.85))
                    .text_color(p.faint)
                    .child(text),
                None => view.p(px(12.0)).child(
                    list(self.lines.clone(), move |index, _, _| {
                        let text = lines
                            .get(index)
                            .map(|line| line.text.clone())
                            .unwrap_or_default();
                        let selected =
                            selection.is_some_and(|(a, b)| index >= a.min(b) && index <= a.max(b));
                        let down = shell.clone();
                        let over = shell.clone();
                        div()
                            .id(index)
                            .cursor_text()
                            .when(selected, |line| line.bg(selection_bg))
                            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                down.update(cx, |shell, cx| {
                                    if let Some(view) = shell.overlay::<LogsView>() {
                                        let anchor = match (event.modifiers.shift, view.selection) {
                                            (true, Some((anchor, _))) => anchor,
                                            _ => index,
                                        };
                                        view.selection = Some((anchor, index));
                                        view.selecting = true;
                                    }
                                    cx.notify();
                                });
                            })
                            .on_mouse_move(move |event: &gpui::MouseMoveEvent, _, cx| {
                                if event.pressed_button != Some(MouseButton::Left) {
                                    return;
                                }
                                over.update(cx, |shell, cx| {
                                    if let Some(view) = shell.overlay::<LogsView>()
                                        && view.selecting
                                        && let Some((anchor, current)) = view.selection
                                        && current != index
                                    {
                                        view.selection = Some((anchor, index));
                                        cx.notify();
                                    }
                                });
                            })
                            .font_family(LOG_FONT)
                            .text_size(rem(0.76))
                            .line_height(relative(1.5))
                            .text_color(text_color)
                            .child(text)
                            .into_any_element()
                    })
                    .size_full(),
                ),
            });

        let session_id = self.session.clone();
        let sessions = (!self.sessions.is_empty()).then(|| {
            let buttons = self
                .sessions
                .iter()
                .zip(&self.labels)
                .map(|(session, label)| {
                    let active = session_id.as_deref() == Some(session.id.as_str());
                    let key = Key::new("log-session", &session.id);
                    let hover = motion::value(key, cx);
                    let id = session.id.clone();
                    let menu_id = session.id.clone();
                    let menu_label = label.clone();
                    let current = session.active;
                    div()
                        .id(ElementId::Name(format!("session-{}", session.id).into()))
                        .flex_none()
                        .relative()
                        .top(px(-hover))
                        .px(px(11.0))
                        .py(px(6.0))
                        .rounded(px(9.0))
                        .border_1()
                        .border_color(if active {
                            p.blue
                        } else {
                            mix(p.line, p.line_strong, hover)
                        })
                        .bg(if active {
                            rgba(0x3b82e8, 0.16)
                        } else {
                            p.surface_2
                        })
                        .text_color(if active {
                            if p.is_light() {
                                p.active_text
                            } else {
                                gpui::rgb(0xdbeafe).into()
                            }
                        } else {
                            mix(p.muted, p.text, hover)
                        })
                        .text_size(rem(0.71))
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .on_hover(move |hovered, window, cx| {
                            motion::hover(key, *hovered, window, cx)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            sound::play(Sound::Tap);
                            if let Some(view) = this.overlay::<LogsView>() {
                                view.session = Some(id.clone());
                            }
                            this.refresh_log(true, cx);
                        }))
                        .when(!current, |button| {
                            button.on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                    this.popup = Some(Popup::LogSession {
                                        session: menu_id.clone(),
                                        label: menu_label.to_string(),
                                        x: f32::from(event.position.x),
                                        y: f32::from(event.position.y),
                                    });
                                    cx.notify();
                                }),
                            )
                        })
                        .child(label.clone())
                });
            div()
                .id("logs-sessions")
                .flex()
                .gap(px(7.0))
                .pt(px(9.0))
                .px(px(1.0))
                .pb(px(2.0))
                .mt(px(7.0))
                .overflow_x_scroll()
                .track_scroll(&self.sessions_scroll)
                .children(buttons)
        });

        let main = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .min_h(px(30.0))
                    .mb(px(10.0))
                    .child(
                        div()
                            .overflow_hidden()
                            .text_size(rem(0.92))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    ),
            )
            .children(meta)
            .child(log)
            .children(sessions);

        let has_logs = !self.summaries.is_empty();
        let height = f32::from(_window.viewport_size().height) * 0.58;
        ModalFrame::new("logs-overlay", 960.0, Shell::closer::<LogsView>(cx))
            .max_width_fraction(0.95)
            .child(modal_head("Логи клиентов", None, cx))
            .when_some(has_logs.then_some(()), |frame, ()| {
                frame.child(
                    div().absolute().top(px(14.0)).right(px(52.0)).child(
                        IconButton::new("logs-clear", Key::of("logs-clear"), IconStyle::Danger)
                            .icon(crate::assets::Icon::Trash, 15.0)
                            .size(30.0, 30.0)
                            .tooltip("Удалить все логи")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_clear_logs(window, cx)
                            })),
                    ),
                )
            })
            .child(
                div()
                    .flex()
                    .gap(px(14.0))
                    .h(px(height))
                    .pt(px(18.0))
                    .px(px(20.0))
                    .pb(px(20.0))
                    .child(side)
                    .child(main),
            )
            .into_any_element()
    }
}

impl Shell {
    pub(crate) fn copy_log_selection(&mut self, all: bool, cx: &mut Context<Self>) -> bool {
        let Some(view) = self.overlay::<LogsView>() else {
            return false;
        };
        if all {
            view.selection = (!view.log.is_empty()).then(|| (0, view.log.len() - 1));
            cx.notify();
            return true;
        }
        let Some((a, b)) = view.selection else {
            return false;
        };
        let text = view.log[a.min(b)..=a.max(b).min(view.log.len().saturating_sub(1))]
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        self.toast("Скопировано", false, cx);
        true
    }

    pub(crate) fn open_logs(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_overlay(Overlay::Logs(LogsView::new()), cx);
        self.refresh_logs_side(cx);
    }

    pub(crate) fn on_logs_changed(&mut self, cx: &mut Context<Self>) {
        if self.has_overlay::<LogsView>() {
            self.refresh_logs_side(cx);
            self.refresh_log(false, cx);
        }
    }

    fn refresh_logs_side(&mut self, cx: &mut Context<Self>) {
        let future = self.store.read(cx).backend.logs();
        cx.spawn(async move |this, cx| {
            let Ok(mut logs) = future.await else {
                return;
            };
            logs.sort_by(|a, b| b.active.cmp(&a.active).then_with(|| a.name.cmp(&b.name)));
            this.update(cx, |shell, cx| {
                let Some(view) = shell.overlay::<LogsView>() else {
                    return;
                };
                view.loaded = true;
                let first = logs.first().map(|l| l.uuid.clone());
                let selected_gone = view
                    .uuid
                    .as_ref()
                    .is_some_and(|uuid| !logs.iter().any(|l| &l.uuid == uuid));
                view.summaries = logs;
                if selected_gone {
                    view.uuid = None;
                    view.session = None;
                    view.clear_log();
                }
                let pick = view.uuid.is_none().then_some(first).flatten();
                cx.notify();
                if let Some(uuid) = pick {
                    shell.select_log(uuid, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn select_log(&mut self, uuid: String, cx: &mut Context<Self>) {
        if let Some(view) = self.overlay::<LogsView>() {
            view.uuid = Some(uuid);
            view.session = None;
        }
        self.refresh_log(true, cx);
    }

    fn refresh_log(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<LogsView>() else {
            return;
        };
        let Some(uuid) = view.uuid.clone() else {
            return;
        };
        let session = view.session.clone();
        let future = self.store.read(cx).backend.log(uuid.clone(), session);
        cx.spawn(async move |this, cx| {
            let Ok(log) = future.await else {
                return;
            };
            this.update(cx, |shell, cx| {
                let Some(view) = shell
                    .overlay::<LogsView>()
                    .filter(|view| view.uuid.as_deref() == Some(uuid.as_str()))
                else {
                    return;
                };
                let unchanged =
                    view.log.len() == log.lines.len() && view.log.last() == log.lines.last();
                if view
                    .session
                    .as_ref()
                    .is_none_or(|id| !log.sessions.iter().any(|s| &s.id == id))
                {
                    view.session = log.sessions.first().map(|s| s.id.clone());
                }
                if force {
                    view.selection = None;
                }
                if force || !unchanged {
                    let old = view.log.len();
                    let new = log.lines.len();
                    if force || new < old {
                        view.lines.reset(new);
                    } else {
                        view.lines.splice(old..old, new - old);
                    }
                }
                view.set_log(log);
                let active = view.current().is_some_and(|s| s.active);
                if active && view.ticker.is_none() {
                    view.ticker = Some(cx.spawn(async move |this, cx| {
                        loop {
                            cx.background_executor().timer(Duration::from_secs(1)).await;
                            if this.update(cx, |_, cx| cx.notify()).is_err() {
                                break;
                            }
                        }
                    }));
                } else if !active {
                    view.ticker = None;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn confirm_clear_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let future = self.store.read(cx).backend.logs_size();
        cx.spawn_in(window, async move |this, cx| {
            let size = future.await.unwrap_or(0);
            this.update_in(cx, |shell, window, cx| {
                let text = format!(
                    "Вы действительно хотите удалить все записанные логи всех аккаунтов на {} MB?",
                    megabytes(size)
                );
                shell.confirm(
                    "Удалить все логи?",
                    text,
                    "Да, удалить",
                    |shell, _, cx| {
                        let future = shell.store.read(cx).backend.clear_logs();
                        cx.spawn(async move |this, cx| {
                            future.await.ok();
                            this.update(cx, |shell, cx| {
                                if let Some(view) = shell.overlay::<LogsView>() {
                                    view.uuid = None;
                                    view.session = None;
                                    view.clear_log();
                                    view.ticker = None;
                                }
                                shell.refresh_logs_side(cx);
                            })
                            .ok();
                        })
                        .detach();
                    },
                    window,
                    cx,
                );
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn render_log_session_menu(
        &mut self,
        session: String,
        label: String,
        x: f32,
        y: f32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let p = cx.palette();
        let menu = menu_surface(p.surface, 11.0, 6.0, cx)
            .min_w(px(190.0))
            .text_size(rem(0.85))
            .child(
                menu_item("delete-session", MenuItem::Danger, &p)
                    .px(px(12.0))
                    .py(px(9.0))
                    .rounded(px(7.0))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        sound::play(Sound::Tap);
                        this.popup = None;
                        let session = session.clone();
                        let text = format!("Вы действительно хотите удалить сессию «{label}»?");
                        this.confirm(
                            "Удалить сессию?",
                            text,
                            "Да, удалить",
                            move |shell, _, cx| {
                                let Some(uuid) =
                                    shell.overlay::<LogsView>().and_then(|v| v.uuid.clone())
                                else {
                                    return;
                                };
                                let future = shell
                                    .store
                                    .read(cx)
                                    .backend
                                    .delete_log_session(uuid, session);
                                cx.spawn(async move |this, cx| {
                                    future.await.ok();
                                    this.update(cx, |shell, cx| {
                                        if let Some(view) = shell.overlay::<LogsView>() {
                                            view.session = None;
                                        }
                                        shell.refresh_log(true, cx);
                                        shell.refresh_logs_side(cx);
                                    })
                                    .ok();
                                })
                                .detach();
                            },
                            window,
                            cx,
                        );
                    }))
                    .child("Удалить сессию"),
            );
        Some(
            deferred(
                div().absolute().left(px(x)).top(px(y)).child(drop_in(
                    "session-menu-in",
                    menu.id("session-menu")
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_card_menu(cx))),
                    None,
                )),
            )
            .with_priority(12)
            .into_any_element(),
        )
    }
}
