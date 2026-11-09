use ac_api::{Account, PunishmentKind, TokenState, Violation};
use gpui::{
    Context, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::assets::{Icon, icon};
use crate::format::{now_secs, relative_time};
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, mix, rem, rgba, shadow};
use crate::ui::{Avatar, Badge, Chip, IconButton, IconStyle, Spinner, fit, with_tooltip};
use crate::views::selection::{PICK, PICK_POP, SELECTING};
use crate::views::shell::{Popup, Shell};

pub const AVATAR_SIZE: f32 = 46.0;

impl Shell {
    pub(crate) fn render_card(
        &mut self,
        account: &Account,
        width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let rank = store.display_rank(account);
        let status = store.online_status(account);
        let uuid = account.uuid.clone();
        let cabinet = store.cabinet_view(&uuid);
        let key = Key::new("card", &uuid);
        let hover = motion::value(key, cx);
        let token = if self.home.debug_expired && account.launchable {
            TokenState::Expired
        } else {
            account.token_state()
        };
        let name_known = !account.name.is_empty();
        let name = if name_known {
            account.name.clone()
        } else {
            format!("{}…", account.short_id())
        };
        let body = width - 144.0;
        let mut title_budget = body - 29.0;
        if let Some(rank) = &rank {
            title_budget -= fit::chip_width(&rank.label, false, window) + 7.0;
        }
        if token == TokenState::Expired {
            title_budget -= 21.0;
        }
        if !account.label.is_empty() {
            title_budget -=
                fit::text_width(&account.label, 10.56, FontWeight::SEMIBOLD, 0.0, window) + 23.0;
        }
        let name_size = if name_known {
            fit::fit_size(&name, 15.2, 11.0, FontWeight::BOLD, title_budget, window)
        } else {
            rem(0.82)
        };
        let time = relative_time(account.last_launched, now_secs())
            .filter(|_| !status.as_ref().is_some_and(|status| status.on));
        let mut meta_budget = body;
        if token == TokenState::Missing {
            meta_budget -= fit::text_width("НЕТ ТОКЕНА", 9.6, FontWeight::BOLD, 0.4, window) + 21.0;
        }
        let punishments: Vec<(PunishmentKind, Violation)> = cabinet
            .status
            .iter()
            .flat_map(|status| {
                [
                    status.ban.clone().map(|ban| (PunishmentKind::Ban, ban)),
                    status.mute.clone().map(|mute| (PunishmentKind::Mute, mute)),
                ]
            })
            .flatten()
            .collect();
        meta_budget -= punishments.len() as f32 * (CHIP_ICON_ONLY + CHIP_GAP);
        let time_width = time.as_ref().map_or(0.0, |time| {
            fit::text_width(time, 11.2, FontWeight::NORMAL, 0.0, window) + 7.0
        });
        let (status_size, show_time) = match &status {
            Some(status) => {
                let with_time = meta_budget - time_width - 14.0;
                if fit::fits(&status.text, 9.0, FontWeight::BOLD, with_time, window) {
                    (
                        fit::fit_size(
                            &status.text,
                            10.56,
                            9.0,
                            FontWeight::BOLD,
                            with_time,
                            window,
                        ),
                        true,
                    )
                } else {
                    (
                        fit::fit_size(
                            &status.text,
                            10.56,
                            8.0,
                            FontWeight::BOLD,
                            meta_budget - 14.0,
                            window,
                        ),
                        false,
                    )
                }
            }
            None => (rem(0.66), true),
        };
        let dot = status.as_ref().and_then(|status| {
            if status.on {
                Some(p.green)
            } else if status.hidden {
                None
            } else {
                Some(p.faint)
            }
        });
        let dot_tip = status.as_ref().map(|s| s.text.clone());
        let pick = motion::value(Key::new(PICK, &uuid), cx);
        let pop = motion::value(Key::new(PICK_POP, &uuid), cx);
        let selecting = motion::value(Key::of(SELECTING), cx);
        let tint = Hsla { a: 1.0, ..p.blue };
        let surface = mix(mix(p.surface, p.surface_2, hover), tint, 0.09 * pick);
        let mark_shown = selecting.max(hover).max(pick).min(1.0);

        let avatar = div()
            .relative()
            .flex_none()
            .child(Avatar::new(&self.store, uuid.clone(), AVATAR_SIZE, 9.0))
            .when_some(dot, |wrap, color| {
                let dot_key = Key::new("card-dot", &uuid);
                let dot = div()
                    .id(ElementId::Name(format!("dot-{uuid}").into()))
                    .absolute()
                    .right(px(-2.0))
                    .bottom(px(-2.0))
                    .size(px(13.0))
                    .rounded_full()
                    .border_2()
                    .border_color(surface)
                    .bg(color);
                let dot = match dot_tip {
                    Some(tip) => with_tooltip(dot, dot_key, tip.into()).into_any_element(),
                    None => dot.into_any_element(),
                };
                wrap.child(dot)
            })
            .when(mark_shown > 0.01, |wrap| {
                let toggled = uuid.clone();
                wrap.child(
                    div()
                        .id(ElementId::Name(format!("pick-{uuid}").into()))
                        .absolute()
                        .left(px(-6.0))
                        .top(px(-6.0))
                        .p(px(2.0))
                        .rounded(px(7.0))
                        .bg(surface)
                        .opacity(mark_shown)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_picked(&toggled, cx);
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(18.0))
                                .rounded(px(5.0))
                                .border(px(1.5))
                                .border_color(mix(p.line_strong, tint, pick))
                                .bg(Hsla { a: pick, ..tint })
                                .when(pop > 0.02, |mark| {
                                    mark.child(icon(Icon::Check, px(13.0 * pop), p.white))
                                }),
                        ),
                )
            });

        let pin_key = Key::new("card-pin", &uuid);
        let pin_hover = motion::value(pin_key, cx);
        let pinned = account.pinned;
        let pin = with_tooltip(
            div()
                .id(ElementId::Name(format!("pin-{uuid}").into()))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(22.0))
                .rounded(px(6.0))
                .bg(rgba(0x000000, 0.0))
                .when(pin_hover > 0.0, |pin| {
                    pin.bg(mix(rgba(0x000000, 0.0), p.surface_2, pin_hover))
                })
                .cursor_pointer()
                .on_hover(move |hovered, window, cx| motion::hover(pin_key, *hovered, window, cx))
                .on_click(cx.listener({
                    let uuid = uuid.clone();
                    move |this, _, _, cx| {
                        cx.stop_propagation();
                        sound::play(Sound::Tap);
                        this.store
                            .update(cx, |store, cx| store.toggle_pin(&uuid, cx));
                    }
                }))
                .child(if pinned {
                    icon(Icon::StarSolid, px(15.0), p.yellow)
                } else {
                    icon(Icon::Star, px(15.0), mix(p.faint, p.muted, pin_hover))
                }),
            pin_key,
            if pinned {
                "Открепить"
            } else {
                "Закрепить"
            }
            .into(),
        );

        let title = div()
            .flex()
            .items_center()
            .gap(px(7.0))
            .min_w_0()
            .child(pin)
            .children(rank.map(|rank| Chip::new(rank.label, rank.color)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .when(name_known, |n| {
                        n.text_size(name_size)
                            .font_weight(FontWeight::BOLD)
                            .whitespace_nowrap()
                    })
                    .when(!name_known, |n| {
                        n.text_size(rem(0.82))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.muted)
                    })
                    .child(name),
            )
            .when(token == TokenState::Expired, |row| {
                let warn_key = Key::new("card-warn", &uuid);
                row.child(with_tooltip(
                    div()
                        .id(ElementId::Name(format!("warn-{uuid}").into()))
                        .flex()
                        .flex_none()
                        .items_center()
                        .opacity(0.85 + 0.15 * hover)
                        .child(icon(Icon::Warn, px(14.0), p.yellow)),
                    warn_key,
                    "Токен истёк - зайди на аккаунт через лаунчер".into(),
                ))
            })
            .when(!account.label.is_empty(), |row| {
                row.child(
                    div()
                        .flex_none()
                        .max_w(px(90.0))
                        .overflow_hidden()
                        .px(px(7.0))
                        .py(px(1.0))
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.surface_2)
                        .text_size(rem(0.66))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.muted)
                        .child(account.label.clone()),
                )
            });

        let meta = div()
            .flex()
            .items_center()
            .gap(px(7.0))
            .mt(px(4.0))
            .min_w_0()
            .when(token == TokenState::Missing, |meta| {
                meta.child(Badge::missing())
            })
            .children(
                punishments
                    .iter()
                    .map(|(kind, violation)| punishment_chip(&uuid, *kind, violation, &p)),
            )
            .when_some(status, |meta, status| {
                meta.child(Badge::status(status.text, status.on).size(status_size))
            })
            .children(time.filter(|_| show_time).map(|time| {
                div()
                    .flex_none()
                    .text_size(rem(0.7))
                    .font(crate::theme::tabular(FontWeight::NORMAL))
                    .text_color(p.faint)
                    .child(time)
            }));

        let play_key = Key::new("card-play", &uuid);
        let (style, tip): (IconStyle, &str) = if account.running {
            (IconStyle::Stop, "Закрыть аккаунт")
        } else if account.launching {
            (IconStyle::Busy, "Запускается…")
        } else {
            (
                IconStyle::Play,
                if account.launchable {
                    "Запуск"
                } else {
                    "Нет токена"
                },
            )
        };
        let mut play = IconButton::new(
            ElementId::Name(format!("play-{uuid}").into()),
            play_key,
            style,
        )
        .on_secondary(cx.listener({
            let uuid = uuid.clone();
            move |this, event: &gpui::ClickEvent, _, cx| {
                let position = event.position();
                this.set_popup(Some(Popup::Card {
                    uuid: uuid.clone(),
                    x: f32::from(position.x),
                    y: f32::from(position.y),
                }));
                cx.notify();
            }
        }))
        .tooltip(tip)
        .on_click(cx.listener({
            let uuid = uuid.clone();
            move |this, _, _, cx| {
                cx.stop_propagation();
                this.store
                    .update(cx, |store, cx| store.toggle_play(&uuid, cx));
            }
        }));
        play = match style {
            IconStyle::Stop => play.content(div().size(px(13.0)).rounded(px(3.0)).bg(p.white)),
            IconStyle::Busy => play.content(Spinner::new(
                ElementId::Name(format!("spin-{uuid}").into()),
                16.0,
            )),
            _ => play.icon(Icon::Play, 20.0),
        };

        let open_uuid = uuid.clone();
        let menu_uuid = uuid.clone();
        div()
            .id(ElementId::Name(format!("card-{uuid}").into()))
            .relative()
            .top(px(-2.0 * hover))
            .flex()
            .items_center()
            .gap(px(13.0))
            .min_h(px(76.0))
            .pl(px(17.0))
            .pr(px(62.0))
            .py(px(13.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(mix(
                mix(p.line, p.line_strong, hover),
                mix(tint, p.blue_hover, hover),
                pick,
            ))
            .bg(surface)
            .shadow(shadow(
                0.0,
                6.0,
                22.0,
                0.0,
                mix(
                    rgba(0x000000, 0.28 * hover),
                    rgba(0x3b82e8, 0.16 + 0.12 * hover),
                    pick,
                ),
            ))
            .cursor_pointer()
            .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    if this.selection_click(&open_uuid, event.modifiers(), cx) {
                        return;
                    }
                    sound::play(Sound::Tap);
                    this.open_account(open_uuid.clone(), window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    let x = f32::from(event.position.x);
                    let y = f32::from(event.position.y);
                    let selection = &this.home.selection;
                    this.popup = Some(
                        if selection.contains(&menu_uuid) && selection.picked.len() > 1 {
                            Popup::Selection { x, y, full: true }
                        } else {
                            Popup::Card {
                                uuid: menu_uuid.clone(),
                                x,
                                y,
                            }
                        },
                    );
                    cx.notify();
                }),
            )
            .child(avatar)
            .child(div().flex_1().min_w_0().child(title).child(meta))
            .child(
                div()
                    .absolute()
                    .right(px(13.0))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .child(play),
            )
    }
}

const CHIP_ICON_ONLY: f32 = 26.5;
const CHIP_GAP: f32 = 7.0;

pub(crate) const ISSUER: &str = "Выдал";

pub(crate) fn punish_title(kind: PunishmentKind) -> &'static str {
    match kind {
        PunishmentKind::Ban => "Бан",
        PunishmentKind::Mute => "Мут",
    }
}

pub(crate) fn punish_lines(
    kind: PunishmentKind,
    violation: &Violation,
) -> Vec<(&'static str, String)> {
    let mut lines = vec![(
        "Срок",
        if violation.permanent {
            "навсегда".to_owned()
        } else if violation.expires_at > 0 {
            format!(
                "до {}",
                crate::format::date_time(violation.expires_at / 1000)
            )
        } else {
            "неизвестно".to_owned()
        },
    )];
    if !violation.permanent && violation.expires_at > 0 {
        let left = violation.expires_at - crate::format::now_millis();
        if left > 0 {
            lines.push(("Осталось", crate::format::span(left)));
        }
    }
    if !violation.reason.is_empty() {
        lines.push(("Правило", violation.reason.clone()));
    }
    if !violation.issued_by_display.is_empty() {
        lines.push((
            ISSUER,
            violation
                .issued_by_display
                .iter()
                .map(|segment| segment.text.as_str())
                .collect(),
        ));
    } else if !violation.issued_by_name.is_empty() {
        lines.push((ISSUER, violation.issued_by_name.clone()));
    }
    if violation.price > 0 {
        lines.push((
            match kind {
                PunishmentKind::Ban => "Стоимость разбана",
                PunishmentKind::Mute => "Стоимость размута",
            },
            violation.price.to_string(),
        ));
    }
    lines
}

fn punish_tooltip(kind: PunishmentKind, violation: &Violation) -> String {
    let mut text = punish_title(kind).to_owned();
    for (label, value) in punish_lines(kind, violation) {
        text.push('\n');
        text.push_str(label);
        text.push_str(": ");
        text.push_str(&value);
    }
    text
}

fn punishment_chip(
    uuid: &str,
    kind: PunishmentKind,
    violation: &Violation,
    p: &crate::theme::Palette,
) -> gpui::Stateful<gpui::Div> {
    let (color, glyph, name) = match kind {
        PunishmentKind::Ban => (p.red, Icon::Gavel, "card-ban"),
        PunishmentKind::Mute => (crate::theme::rgb(0xe08636), Icon::Muted, "card-mute"),
    };
    let chip = div()
        .id(ElementId::Name(format!("{name}-{uuid}").into()))
        .flex()
        .flex_none()
        .items_center()
        .px(px(7.0))
        .py(px(2.0))
        .rounded_full()
        .bg(gpui::Hsla { a: 0.15, ..color })
        .child(icon(glyph, px(12.5), color));
    with_tooltip(
        chip,
        Key::new(name, uuid),
        punish_tooltip(kind, violation).into(),
    )
}
