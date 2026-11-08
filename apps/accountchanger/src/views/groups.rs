use std::collections::BTreeSet;

use ac_api::{
    Account, ClientToggle, GROUP_NAME_MAX_CHARS, Group, LaunchSettings, RAM_DEFAULT, RAM_MAX,
    RAM_MIN, RAM_STEP, snap_ram,
};
use gpui::{
    AnyElement, AppContext, Context, ElementId, Entity, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Radians, Render, ScrollHandle, StatefulInteractiveElement, Styled,
    Transformation, Window, div, prelude::FluentBuilder, px,
};

use crate::assets::{Icon, icon};
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, alpha, mix, rem, rgba};
use crate::ui::{
    Avatar, Badge, Button, ButtonStyle, Checkbox, Chip, InputEvent, ModalFrame, ScrollArea,
    Segmented, Slider, TextInput, Toggle, modal_head, spaced,
};
use crate::views::account::ram_number;
use crate::views::settings::small_button;
use crate::views::shell::{Overlay, Shell};
use crate::views::widgets::field_label;

pub struct GroupsView {
    expanded: Option<String>,
    dragging: Option<String>,
    order: Vec<String>,
    scroll: ScrollHandle,
}

pub struct AddMembersView {
    group: String,
    search: Entity<TextInput>,
    scroll: ScrollHandle,
}

pub struct LaunchSettingsView {
    members: Vec<String>,
    picked: BTreeSet<String>,
    ram: u32,
    ram_input: Entity<TextInput>,
    chunks: Entity<TextInput>,
    fps: Entity<TextInput>,
    settings: LaunchSettings,
    scroll: ScrollHandle,
}

struct GroupDrag(String);

struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

const TOGGLES: [(&str, &str); 5] = [
    ("minGraphics", "Минимальная графика"),
    ("fullscreen", "Полный экран"),
    ("discordRPC", "Discord RPC"),
    ("autoEnter", "Автовход в игру"),
    ("debugMode", "Режим отладки"),
];

fn toggle_value<'a>(settings: &'a mut LaunchSettings, key: &str) -> &'a mut bool {
    match key {
        "minGraphics" => &mut settings.min_graphics,
        "fullscreen" => &mut settings.fullscreen,
        "discordRPC" => &mut settings.discord_rpc,
        "autoEnter" => &mut settings.auto_enter,
        _ => &mut settings.debug_mode,
    }
}

fn ordered(groups: &[Group], order: &[String]) -> Vec<Group> {
    let mut list: Vec<Group> = order
        .iter()
        .filter_map(|id| groups.iter().find(|g| &g.id == id).cloned())
        .collect();
    list.extend(groups.iter().filter(|g| !order.contains(&g.id)).cloned());
    list
}

impl GroupsView {
    pub fn render(
        &mut self,
        shell: &mut Shell,
        _window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let store = shell.store.read(cx);
        let groups = ordered(&store.groups, &self.order);
        let accounts: Vec<Account> = store.accounts.clone();
        let rows: Vec<AnyElement> = groups
            .iter()
            .map(|group| {
                let expanded = self.expanded.as_deref() == Some(group.id.as_str());
                let dragging = self.dragging.as_deref() == Some(group.id.as_str());
                let id = group.id.clone();
                let row_key = Key::new("gm-group", &group.id);
                let hover = motion::value(row_key, cx);
                let pin_key = Key::new("gm-pin", &group.id);
                let pin_hover = motion::value(pin_key, cx);
                let toggle_key = Key::new("gm-toggle", &group.id);
                let toggle_hover = motion::value(toggle_key, cx);
                let pinned = group.pinned;
                let head = div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(13.0))
                    .py(px(11.0))
                    .child(div().px(px(2.0)).cursor_grab().child(icon(
                        Icon::Grip,
                        px(14.0),
                        p.faint,
                    )))
                    .child(
                        div()
                            .id(ElementId::Name(format!("gm-pin-{id}").into()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(30.0))
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(if pinned {
                                alpha(p.yellow, 0.45)
                            } else {
                                mix(p.line, p.line_strong, pin_hover)
                            })
                            .bg(if pinned {
                                alpha(p.yellow, 0.12)
                            } else {
                                rgba(0x000000, 0.0)
                            })
                            .cursor_pointer()
                            .on_hover(move |hovered, window, cx| {
                                motion::hover(pin_key, *hovered, window, cx)
                            })
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| {
                                    sound::play(Sound::Tap);
                                    this.store.update(cx, |store, cx| {
                                        let future =
                                            store.backend.set_group_pinned(id.clone(), !pinned);
                                        store.act(cx, future, "");
                                    });
                                }
                            }))
                            .child(if pinned {
                                icon(Icon::StarSolid, px(15.0), p.yellow)
                            } else {
                                icon(Icon::Star, px(15.0), mix(p.faint, p.muted, pin_hover))
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_size(rem(0.92))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(group.name.clone()),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(format!("gm-toggle-{id}").into()))
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .px(px(11.0))
                            .py(px(4.0))
                            .rounded(px(20.0))
                            .border_1()
                            .border_color(if expanded {
                                p.blue
                            } else {
                                mix(p.line, p.line_strong, toggle_hover)
                            })
                            .text_color(if expanded {
                                p.text
                            } else {
                                mix(p.muted, p.text, toggle_hover)
                            })
                            .text_size(rem(0.74))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .on_hover(move |hovered, window, cx| {
                                motion::hover(toggle_key, *hovered, window, cx)
                            })
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| {
                                    sound::play(Sound::Tap);
                                    if let Some(view) = this.overlay::<GroupsView>() {
                                        view.expanded =
                                            if view.expanded.as_deref() == Some(id.as_str()) {
                                                None
                                            } else {
                                                Some(id.clone())
                                            };
                                    }
                                    cx.notify();
                                }
                            }))
                            .child(format!("{} акк.", group.members.len()))
                            .child(
                                icon(
                                    Icon::Caret,
                                    px(15.0),
                                    if expanded { p.text } else { p.muted },
                                )
                                .with_transformation(
                                    Transformation::rotate(Radians(if expanded {
                                        std::f32::consts::PI
                                    } else {
                                        0.0
                                    })),
                                ),
                            ),
                    )
                    .child(
                        small_button_keyed("gm-rename", &id, "Переименовать", false, cx).on_click(
                            cx.listener({
                                let group = group.clone();
                                move |this, _, window, cx| {
                                    sound::play(Sound::Tap);
                                    let id = group.id.clone();
                                    this.prompt(
                                        "Новое название группы",
                                        group.name.clone(),
                                        GROUP_NAME_MAX_CHARS,
                                        move |this, name, _, cx| {
                                            this.store.update(cx, |store, cx| {
                                                let future = store.backend.rename_group(id, name);
                                                store.act(cx, future, "");
                                            });
                                        },
                                        window,
                                        cx,
                                    );
                                }
                            }),
                        ),
                    )
                    .child(
                        small_button_keyed("gm-delete", &id, "Удалить", true, cx).on_click(
                            cx.listener({
                                let group = group.clone();
                                move |this, _, window, cx| {
                                    sound::play(Sound::Tap);
                                    let id = group.id.clone();
                                    this.confirm(
                                        "Удалить группу?",
                                        format!(
                                            "Вы уверены, что хотите удалить группу «{}»?",
                                            group.name
                                        ),
                                        "Да, удалить",
                                        move |this, _, cx| {
                                            if this.home.group.as_deref() == Some(id.as_str()) {
                                                this.select_group(None, cx);
                                            }
                                            if let Some(view) = this.overlay::<GroupsView>()
                                                && view.expanded.as_deref() == Some(id.as_str())
                                            {
                                                view.expanded = None;
                                            }
                                            this.store.update(cx, |store, cx| {
                                                let future = store.backend.delete_group(id);
                                                store.act(cx, future, "");
                                            });
                                        },
                                        window,
                                        cx,
                                    );
                                }
                            }),
                        ),
                    );
                let members = expanded.then(|| {
                    let list: Vec<&Account> = group
                        .members
                        .iter()
                        .filter_map(|uuid| accounts.iter().find(|a| &a.uuid == uuid))
                        .collect();
                    let add_key = Key::new("gm-add", &group.id);
                    let add_hover = motion::value(add_key, cx);
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .px(px(13.0))
                        .pt(px(4.0))
                        .pb(px(13.0))
                        .mt(px(2.0))
                        .border_t_1()
                        .border_color(p.line)
                        .when(list.is_empty(), |panel| {
                            panel.child(
                                div()
                                    .py(px(10.0))
                                    .px(px(2.0))
                                    .text_size(rem(0.8))
                                    .text_color(p.muted)
                                    .child("В группе нет аккаунтов"),
                            )
                        })
                        .children(list.into_iter().map(|account| {
                            let remove_key = Key::new("gm-remove", (&group.id, &account.uuid));
                            let remove_hover = motion::value(remove_key, cx);
                            let target = (group.id.clone(), account.uuid.clone());
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .px(px(8.0))
                                .py(px(6.0))
                                .rounded(px(9.0))
                                .border_1()
                                .border_color(p.line)
                                .bg(p.surface)
                                .child(Avatar::new(&shell.store, account.uuid.clone(), 26.0, 6.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .text_size(rem(0.84))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(account.display_name().to_owned()),
                                )
                                .child(
                                    div()
                                        .id(ElementId::Name(
                                            format!("gm-remove-{}-{}", target.0, target.1).into(),
                                        ))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(26.0))
                                        .rounded(px(7.0))
                                        .border_1()
                                        .border_color(mix(p.line, p.red, remove_hover))
                                        .bg(mix(
                                            rgba(0x000000, 0.0),
                                            alpha(p.red, 0.12),
                                            remove_hover,
                                        ))
                                        .cursor_pointer()
                                        .on_hover(move |hovered, window, cx| {
                                            motion::hover(remove_key, *hovered, window, cx)
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            sound::play(Sound::Tap);
                                            let (group, uuid) = target.clone();
                                            this.store.update(cx, |store, cx| {
                                                let future =
                                                    store.backend.remove_group_member(group, uuid);
                                                store.act(cx, future, "");
                                            });
                                        }))
                                        .child(icon(
                                            Icon::XMark,
                                            px(13.0),
                                            mix(p.muted, p.red, remove_hover),
                                        )),
                                )
                        }))
                        .child(
                            div()
                                .id(ElementId::Name(format!("gm-add-{id}").into()))
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(7.0))
                                .mt(px(3.0))
                                .p(px(9.0))
                                .rounded(px(9.0))
                                .border_1()
                                .border_dashed()
                                .border_color(mix(p.line_strong, p.blue, add_hover))
                                .bg(mix(rgba(0x000000, 0.0), rgba(0x3b82e8, 0.07), add_hover))
                                .text_color(mix(p.muted, p.blue, add_hover))
                                .text_size(rem(0.82))
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .on_hover(move |hovered, window, cx| {
                                    motion::hover(add_key, *hovered, window, cx)
                                })
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, window, cx| {
                                        sound::play(Sound::Tap);
                                        this.open_add_members(Some(id.clone()), window, cx);
                                    }
                                }))
                                .child(icon(
                                    Icon::PlusBold,
                                    px(16.0),
                                    mix(p.muted, p.blue, add_hover),
                                ))
                                .child("Добавить аккаунты"),
                        )
                });
                div()
                    .id(ElementId::Name(format!("gm-group-{id}").into()))
                    .flex()
                    .flex_col()
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(if dragging {
                        p.blue
                    } else {
                        mix(p.line, p.line_strong, hover)
                    })
                    .bg(p.surface_2)
                    .overflow_hidden()
                    .when(dragging, |row| row.opacity(0.55))
                    .on_hover(move |hovered, window, cx| {
                        motion::hover(row_key, *hovered, window, cx)
                    })
                    .on_drag(GroupDrag(id.clone()), {
                        let id = id.clone();
                        let shell = cx.entity();
                        move |_, _, _, cx| {
                            let id = id.clone();
                            shell.update(cx, |this, cx| {
                                if let Some(view) = this.overlay::<GroupsView>() {
                                    view.dragging = Some(id);
                                }
                                cx.notify();
                            });
                            cx.new(|_| DragGhost)
                        }
                    })
                    .on_drag_move::<GroupDrag>(cx.listener({
                        let id = id.clone();
                        move |this, event: &gpui::DragMoveEvent<GroupDrag>, _, cx| {
                            let dragged = event.drag(cx).0.clone();
                            if dragged == id || !event.bounds.contains(&event.event.position) {
                                return;
                            }
                            let Some(view) = this.overlay::<GroupsView>() else {
                                return;
                            };
                            let (Some(from), Some(to)) = (
                                view.order.iter().position(|g| *g == dragged),
                                view.order.iter().position(|g| *g == id),
                            ) else {
                                return;
                            };
                            let moved = view.order.remove(from);
                            view.order.insert(to, moved);
                            cx.notify();
                        }
                    }))
                    .on_drop::<GroupDrag>(
                        cx.listener(|this, _: &GroupDrag, _, cx| this.finish_group_drag(cx)),
                    )
                    .child(head)
                    .children(members)
                    .into_any_element()
            })
            .collect();
        let body = if groups.is_empty() {
            div()
                .py(px(34.0))
                .px(px(10.0))
                .text_center()
                .text_size(rem(0.85))
                .text_color(p.muted)
                .child("Групп пока нет. Создай первую.")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(9.0))
                .px(px(20.0))
                .py(px(2.0))
                .children(rows)
                .into_any_element()
        };
        let height = f32::from(_window.viewport_size().height) * 0.58;
        ModalFrame::new("groups-overlay", 580.0, Shell::closer::<GroupsView>(cx))
            .max_width_fraction(0.94)
            .padding(0.0, 0.0, 20.0, 0.0)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .pt(px(20.0))
                    .pl(px(20.0))
                    .pr(px(58.0))
                    .pb(px(14.0))
                    .border_b_1()
                    .border_color(p.line)
                    .child(
                        div()
                            .text_size(rem(1.15))
                            .font_weight(FontWeight::BOLD)
                            .child("Управление группами"),
                    )
                    .child(
                        Button::new("groups-create", "Создать группу", ButtonStyle::Primary)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_group_flow(Vec::new(), window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .mt(px(16.0))
                    .max_h(px(height))
                    .flex()
                    .flex_col()
                    .on_mouse_up(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.finish_group_drag(cx)),
                    )
                    .child(
                        ScrollArea::new("groups-list", &self.scroll)
                            .fixed()
                            .child(body),
                    ),
            )
            .into_any_element()
    }
}

fn small_button_keyed(
    scope: &'static str,
    id: &str,
    label: &'static str,
    danger: bool,
    cx: &mut Context<Shell>,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.palette();
    let key = Key::new(scope, id);
    let hover = motion::value(key, cx);
    let (fg, border, bg) = if danger {
        (
            mix(p.muted, p.red, hover),
            mix(p.line, p.red, hover),
            mix(rgba(0x000000, 0.0), alpha(p.red, 0.1), hover),
        )
    } else {
        (
            mix(p.muted, p.text, hover),
            mix(p.line, p.line_strong, hover),
            mix(rgba(0x000000, 0.0), alpha(p.white, 0.04), hover),
        )
    };
    div()
        .id(ElementId::Name(format!("{scope}-{id}").into()))
        .flex_none()
        .px(px(11.0))
        .py(px(6.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(fg)
        .text_size(rem(0.78))
        .cursor_pointer()
        .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
        .child(label)
}

impl AddMembersView {
    pub fn render(
        &mut self,
        shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let store = shell.store.read(cx);
        let Some(group) = store.group(&self.group).cloned() else {
            return div().into_any_element();
        };
        let query = self.search.read(cx).text().trim().to_lowercase();
        let candidates: Vec<Account> = store
            .accounts
            .iter()
            .filter(|a| !group.members.contains(&a.uuid))
            .filter(|a| {
                query.is_empty()
                    || if a.name.is_empty() { &a.uuid } else { &a.name }
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect();
        let focused = self.search.read(cx).is_focused(window);
        let cards: Vec<AnyElement> = candidates
            .iter()
            .map(|account| {
                let store = shell.store.read(cx);
                let rank = store.display_rank(account);
                let status = store.online_status(account);
                let key = Key::new("picker", &account.uuid);
                let hover = motion::value(key, cx);
                let tone = rank
                    .as_ref()
                    .and_then(|r| crate::theme::parse_hex(&r.color))
                    .unwrap_or(p.blue);
                let uuid = account.uuid.clone();
                let group_id = group.id.clone();
                let dot = status.as_ref().and_then(|s| {
                    if s.on {
                        Some(p.green)
                    } else if s.hidden {
                        None
                    } else {
                        Some(p.faint)
                    }
                });
                div()
                    .id(ElementId::Name(format!("picker-{uuid}").into()))
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(13.0))
                    .min_h(px(76.0))
                    .pl(px(17.0))
                    .pr(px(62.0))
                    .py(px(13.0))
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(mix(p.line, alpha(tone, 0.55), hover))
                    .bg(mix(p.surface, p.surface_2, hover))
                    .cursor_pointer()
                    .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        sound::play(Sound::Confirm);
                        let (group, uuid) = (group_id.clone(), uuid.clone());
                        this.store.update(cx, |store, cx| {
                            let future = store.backend.add_group_member(group, uuid);
                            store.act(cx, future, "");
                        });
                    }))
                    .child(
                        div()
                            .relative()
                            .child(Avatar::new(&shell.store, account.uuid.clone(), 46.0, 9.0))
                            .children(dot.map(|color| {
                                div()
                                    .absolute()
                                    .right(px(-2.0))
                                    .bottom(px(-2.0))
                                    .size(px(13.0))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(mix(p.surface, p.surface_2, hover))
                                    .bg(color)
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(7.0))
                                    .children(rank.map(|rank| Chip::new(rank.label, rank.color)))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .text_size(rem(0.95))
                                            .font_weight(FontWeight::BOLD)
                                            .child(account.display_name().to_owned()),
                                    ),
                            )
                            .child(
                                div().flex().mt(px(4.0)).child(match account.token_state() {
                                    ac_api::TokenState::Missing => {
                                        Badge::missing().into_any_element()
                                    }
                                    ac_api::TokenState::Expired => {
                                        Badge::new("истёк", crate::ui::BadgeTone::Expired)
                                            .into_any_element()
                                    }
                                    ac_api::TokenState::Ready => {
                                        Badge::new("готов", crate::ui::BadgeTone::Ready)
                                            .into_any_element()
                                    }
                                }),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .right(px(13.0))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(40.0))
                                    .rounded(px(12.0))
                                    .border_1()
                                    .border_color(p.line_strong)
                                    .bg(mix(p.blue, p.blue_hover, hover))
                                    .child(icon(Icon::PlusBold, px(20.0), p.white)),
                            ),
                    )
                    .into_any_element()
            })
            .collect();
        let list = if cards.is_empty() {
            div()
                .py(px(34.0))
                .px(px(10.0))
                .text_center()
                .text_size(rem(0.85))
                .text_color(p.muted)
                .child("Все аккаунты уже в группе")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .p(px(2.0))
                .pr(px(12.0))
                .children(cards)
                .into_any_element()
        };
        let height = f32::from(window.viewport_size().height) * 0.56;
        ModalFrame::new(
            "addmembers-overlay",
            640.0,
            Shell::closer::<AddMembersView>(cx),
        )
        .max_width_fraction(0.94)
        .padding(0.0, 20.0, 20.0, 20.0)
        .child(div().mx(px(-20.0)).child(modal_head(
            format!("Добавить в «{}»", group.name),
            None,
            cx,
        )))
        .child(
            div()
                .flex()
                .items_center()
                .mt(px(16.0))
                .mb(px(12.0))
                .h(px(41.69))
                .px(px(13.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(if focused { p.line_strong } else { p.line })
                .bg(p.surface)
                .text_size(rem(0.875))
                .child(self.search.clone()),
        )
        .child(
            div().max_h(px(height)).flex().flex_col().child(
                ScrollArea::new("addmembers-list", &self.scroll)
                    .fixed()
                    .child(list),
            ),
        )
        .into_any_element()
    }
}

impl LaunchSettingsView {
    pub fn render(
        &mut self,
        shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let store = shell.store.read(cx);
        let members: Vec<Account> = self
            .members
            .iter()
            .filter_map(|uuid| store.account(uuid).cloned())
            .collect();
        let ram_focused = self.ram_input.read(cx).is_focused(window);
        let minimal = self.settings.minimal;

        let ram_row = div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .mb(px(16.0))
            .child(field_label("RAM", cx))
            .child(
                div().flex_1().min_w_0().flex().child(
                    Slider::new("group-ram-slider", self.ram, RAM_MIN, RAM_MAX, RAM_STEP)
                        .on_input(
                            cx.listener(|this, value: &u32, _, cx| this.set_group_ram(*value, cx)),
                        )
                        .on_commit(
                            cx.listener(|this, value: &u32, _, cx| this.set_group_ram(*value, cx)),
                        ),
                ),
            )
            .child(ram_number(&self.ram_input, ram_focused, false, "group", cx));

        let toggles = div().flex().flex_col().gap(px(2.0)).mb(px(16.0)).children(
            TOGGLES.iter().enumerate().map(|(index, (key, label))| {
                let mut settings = self.settings.clone();
                let on = *toggle_value(&mut settings, key);
                let key = *key;
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .px(px(4.0))
                    .py(px(9.0))
                    .when(index + 1 < TOGGLES.len(), |row| {
                        row.border_b_1().border_color(p.line)
                    })
                    .child(
                        div()
                            .text_size(rem(0.88))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(*label),
                    )
                    .child(
                        Toggle::new(
                            ElementId::Name(format!("ls-{key}").into()),
                            Key::new("ls-toggle", key),
                            on,
                        )
                        .on_toggle(cx.listener(
                            move |this, next: &bool, _, cx| {
                                if let Some(view) = this.overlay::<LaunchSettingsView>() {
                                    *toggle_value(&mut view.settings, key) = *next;
                                }
                                cx.notify();
                            },
                        )),
                    )
            }),
        );

        let client_row = |label: &'static str, last: bool| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .px(px(4.0))
                .py(px(9.0))
                .when(!last, |row| row.border_b_1().border_color(p.line))
                .child(
                    div()
                        .text_size(rem(0.88))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
        };
        let number = |input: &Entity<TextInput>, window: &Window, cx: &gpui::App| {
            let focused = input.read(cx).is_focused(window);
            div()
                .w(px(96.0))
                .flex()
                .px(px(8.0))
                .py(px(6.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(if focused { p.blue } else { p.line })
                .bg(p.surface_2)
                .text_size(rem(0.82))
                .font_weight(FontWeight::SEMIBOLD)
                .child(input.clone())
        };
        let segment = |id: &'static str, value: ClientToggle, cx: &mut Context<Shell>| {
            let selected = ClientToggle::ALL
                .iter()
                .position(|v| *v == value)
                .unwrap_or(0);
            Segmented::new(
                id,
                vec!["Не менять".into(), "Выкл".into(), "Вкл".into()],
                selected,
            )
            .on_select(cx.listener(move |this, index: &usize, _, cx| {
                if let Some(view) = this.overlay::<LaunchSettingsView>() {
                    let value = ClientToggle::ALL[*index];
                    if id == "ls-animations" {
                        view.settings.animations = value;
                    } else {
                        view.settings.fast_render = value;
                    }
                }
                cx.notify();
            }))
        };
        let lockable = div()
            .flex()
            .flex_col()
            .when(minimal, |block| block.opacity(0.4))
            .child(client_row("Прогрузка чанков", false).child(number(&self.chunks, window, cx)))
            .child(client_row("Макс. FPS", false).child(number(&self.fps, window, cx)))
            .child(client_row("Анимации", false).child(segment(
                "ls-animations",
                self.settings.animations,
                cx,
            )))
            .child(client_row("Быстрый рендер", true).child(segment(
                "ls-fast-render",
                self.settings.fast_render,
                cx,
            )));
        let client = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .mb(px(16.0))
            .child(client_row("Минимальные настройки", false).child(
                Toggle::new("ls-minimal", Key::of("ls-minimal"), minimal).on_toggle(cx.listener(
                    |this, next: &bool, _, cx| {
                        let next = *next;
                        if let Some(view) = this.overlay::<LaunchSettingsView>() {
                            view.settings.minimal = next;
                            if next {
                                view.ram = RAM_MIN;
                                view.ram_input.update(cx, |input, cx| {
                                    input.set_text(RAM_MIN.to_string(), cx)
                                });
                            }
                        }
                        cx.notify();
                    },
                )),
            ))
            .child(lockable);

        let all = !members.is_empty() && self.picked.len() == members.len();
        let picks = if members.is_empty() {
            div()
                .py(px(34.0))
                .text_center()
                .text_size(rem(0.85))
                .text_color(p.muted)
                .child("В группе нет аккаунтов")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .mb(px(6.0))
                .children(members.iter().map(|account| {
                    let on = self.picked.contains(&account.uuid);
                    let uuid = account.uuid.clone();
                    div()
                        .id(ElementId::Name(format!("ram-pick-{uuid}").into()))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .px(px(10.0))
                        .py(px(8.0))
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(if on { alpha(p.blue, 0.5) } else { p.line })
                        .bg(p.surface_2)
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            sound::play(Sound::Tap);
                            if let Some(view) = this.overlay::<LaunchSettingsView>()
                                && !view.picked.remove(&uuid)
                            {
                                view.picked.insert(uuid.clone());
                            }
                            cx.notify();
                        }))
                        .child(Checkbox::new(on))
                        .child(Avatar::new(&shell.store, account.uuid.clone(), 26.0, 6.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .text_size(rem(0.85))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(account.display_name().to_owned()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(rem(0.72))
                                .text_color(p.muted)
                                .child(if account.launch.auto_ram() {
                                    "авто".to_owned()
                                } else {
                                    format!("{} МБ", account.launch.ram)
                                }),
                        )
                }))
                .into_any_element()
        };
        let pick_head = div()
            .flex()
            .items_center()
            .justify_between()
            .mb(px(8.0))
            .child(
                div()
                    .text_size(rem(0.72))
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.faint)
                    .child(spaced("ПРИМЕНИТЬ К АККАУНТАМ", 0.4)),
            )
            .child(
                small_button("ram-select-all", "Выбрать все", false, cx).on_click(cx.listener(
                    move |this, _, _, cx| {
                        sound::play(Sound::Tap);
                        if let Some(view) = this.overlay::<LaunchSettingsView>() {
                            view.picked = if all {
                                BTreeSet::new()
                            } else {
                                view.members.iter().cloned().collect()
                            };
                        }
                        cx.notify();
                    },
                )),
            );
        let max_height = f32::from(window.viewport_size().height) * 0.9;
        ModalFrame::new(
            "ram-overlay",
            680.0,
            Shell::closer::<LaunchSettingsView>(cx),
        )
        .max_width_fraction(0.94)
        .child(
            div()
                .max_h(px(max_height))
                .flex()
                .flex_col()
                .child(modal_head(
                    "Настройки запуска",
                    Some("Применяются при запуске выбранных аккаунтов".into()),
                    cx,
                ))
                .child(
                    div().flex_1().min_h_0().flex().flex_col().child(
                        ScrollArea::new("ram-body", &self.scroll).child(
                            div()
                                .pt(px(18.0))
                                .px(px(22.0))
                                .pb(px(8.0))
                                .child(ram_row)
                                .child(toggles)
                                .child(client)
                                .child(pick_head)
                                .child(picks),
                        ),
                    ),
                )
                .child(
                    div()
                        .flex_none()
                        .pt(px(14.0))
                        .px(px(22.0))
                        .pb(px(18.0))
                        .border_t_1()
                        .border_color(p.line)
                        .bg(p.surface)
                        .child(
                            Button::new("ram-apply", "Применить", ButtonStyle::Primary)
                                .full()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.apply_launch_settings(window, cx)
                                })),
                        ),
                ),
        )
        .into_any_element()
    }
}

impl Shell {
    pub(crate) fn open_groups(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let order = self
            .store
            .read(cx)
            .groups
            .iter()
            .map(|g| g.id.clone())
            .collect();
        self.push_overlay(
            Overlay::Groups(GroupsView {
                expanded: None,
                dragging: None,
                order,
                scroll: ScrollHandle::new(),
            }),
            cx,
        );
    }

    pub(crate) fn sync_group_order(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .store
            .read(cx)
            .groups
            .iter()
            .map(|g| g.id.clone())
            .collect();
        if let Some(view) = self.overlay::<GroupsView>()
            && view.dragging.is_none()
        {
            view.order = ids;
        }
    }

    fn finish_group_drag(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<GroupsView>() else {
            return;
        };
        if view.dragging.take().is_none() {
            return;
        }
        let ids = view.order.clone();
        self.store.update(cx, |store, cx| {
            let future = store.backend.reorder_groups(ids);
            store.act(cx, future, "");
        });
        cx.notify();
    }

    pub(crate) fn create_group_flow(
        &mut self,
        members: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prompt(
            "Название новой группы",
            "",
            GROUP_NAME_MAX_CHARS,
            move |this, name, _, cx| {
                let backend = this.store.read(cx).backend.clone();
                let future = backend.create_group(name);
                cx.spawn(async move |this, cx| {
                    let result = future.await;
                    if let Ok(group) = &result {
                        for uuid in &members {
                            backend
                                .add_group_member(group.id.clone(), uuid.clone())
                                .await
                                .ok();
                        }
                    }
                    this.update(cx, |shell, cx| match result {
                        Ok(mut group) => {
                            if group.members.is_empty() {
                                group.members = members;
                            }
                            shell.store.update(cx, |store, cx| {
                                if store.group(&group.id).is_none() {
                                    store.groups.push(group.clone());
                                }
                                store.reload_groups(cx);
                            });
                            shell.select_group(Some(group.id), cx);
                            shell.toast("Группа создана", false, cx);
                        }
                        Err(error) => shell.toast(error.message().to_owned(), true, cx),
                    })
                    .ok();
                })
                .detach();
            },
            window,
            cx,
        );
    }

    pub(crate) fn open_add_members(
        &mut self,
        group: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(group) = group.or_else(|| self.home.group.clone()) else {
            return;
        };
        let search = cx.new(|cx| TextInput::new("Поиск по нику…", window, cx));
        cx.subscribe(&search, |_, _, event, cx| {
            if *event == InputEvent::Changed {
                cx.notify();
            }
        })
        .detach();
        search.read(cx).focus(window);
        self.push_overlay(
            Overlay::AddMembers(AddMembersView {
                group,
                search,
                scroll: ScrollHandle::new(),
            }),
            cx,
        );
    }

    pub(crate) fn open_launch_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let members: Vec<String> = self
            .active_group(store)
            .map(|group| {
                group
                    .members
                    .iter()
                    .filter(|uuid| store.account(uuid).is_some())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let numeric =
            |placeholder: &'static str, max: usize, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    TextInput::new(placeholder, window, cx)
                        .digits()
                        .max_chars(max)
                        .align_right()
                })
            };
        let ram_input = numeric("", 5, window, cx);
        ram_input.update(cx, |input, cx| input.set_text(RAM_DEFAULT.to_string(), cx));
        cx.subscribe(&ram_input, |this, input, event, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                let value = input.read(cx).text().parse().unwrap_or(RAM_MIN);
                this.set_group_ram(value, cx);
            }
        })
        .detach();
        let chunks = numeric("", 2, window, cx);
        cx.subscribe(&chunks, |_, input, event, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                let raw: u32 = input.read(cx).text().parse().unwrap_or(0);
                let value = if raw > 0 { raw.clamp(2, 32) } else { 0 };
                input.update(cx, |input, cx| {
                    input.set_text(
                        if value > 0 {
                            value.to_string()
                        } else {
                            String::new()
                        },
                        cx,
                    )
                });
            }
        })
        .detach();
        let fps = numeric("", 3, window, cx);
        cx.subscribe(&fps, |_, input, event, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                let raw: u32 = input.read(cx).text().parse().unwrap_or(0);
                let value = if raw > 0 { raw.clamp(5, 260) } else { 0 };
                input.update(cx, |input, cx| {
                    input.set_text(
                        if value > 0 {
                            value.to_string()
                        } else {
                            String::new()
                        },
                        cx,
                    )
                });
            }
        })
        .detach();
        self.push_overlay(
            Overlay::LaunchSettings(LaunchSettingsView {
                picked: members.iter().cloned().collect(),
                members,
                ram: RAM_DEFAULT,
                ram_input,
                chunks,
                fps,
                settings: LaunchSettings::default(),
                scroll: ScrollHandle::new(),
            }),
            cx,
        );
    }

    fn set_group_ram(&mut self, value: u32, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<LaunchSettingsView>() else {
            return;
        };
        let ram = snap_ram(value);
        view.ram = ram;
        view.ram_input
            .update(cx, |input, cx| input.set_text(ram.to_string(), cx));
        cx.notify();
    }

    pub(crate) fn step_group_ram(&mut self, direction: i32, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<LaunchSettingsView>() else {
            return;
        };
        let next = (view.ram as i64 + direction as i64 * RAM_STEP as i64).max(0) as u32;
        self.set_group_ram(next, cx);
    }

    fn apply_launch_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<LaunchSettingsView>() else {
            return;
        };
        let uuids: Vec<String> = view.picked.iter().cloned().collect();
        if uuids.is_empty() {
            self.toast("Выбери хотя бы один аккаунт", true, cx);
            return;
        }
        let chunks: u32 = view.chunks.read(cx).text().parse().unwrap_or(0);
        let fps: u32 = view.fps.read(cx).text().parse().unwrap_or(0);
        let settings = LaunchSettings {
            ram: snap_ram(view.ram),
            render_distance: chunks.min(32),
            max_fps: fps.min(260),
            ..view.settings.clone()
        };
        let count = uuids.len();
        let future = self
            .store
            .read(cx)
            .backend
            .set_launch_settings(uuids, settings);
        cx.spawn_in(window, async move |this, cx| {
            let result = future.await;
            this.update_in(cx, |shell, window, cx| match result {
                Ok(()) => {
                    shell.toast(format!("Настройки применены · {count} акк."), false, cx);
                    shell.close::<LaunchSettingsView>(window, cx);
                }
                Err(error) => shell.toast(error.message().to_owned(), true, cx),
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn group_launch_clicked(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let progress = store.group_launch.clone();
        let Some(group) = self.active_group(store).cloned() else {
            return;
        };
        self.store.update(cx, |store, cx| {
            if !progress.active {
                let future = store.backend.launch_group(group.id);
                store.act(cx, future, "Запускаю аккаунты группы поочерёдно…");
            } else if progress.paused {
                let future = store.backend.resume_group_launch();
                store.fetch(cx, future, |_, _, _| {});
            } else {
                let future = store.backend.pause_group_launch();
                store.fetch(cx, future, |_, _, _| {});
            }
        });
    }

    pub(crate) fn confirm_close_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group) = self.active_group(self.store.read(cx)).cloned() else {
            return;
        };
        self.confirm(
            "Закрыть все аккаунты?",
            format!(
                "Будут закрыты все запущенные на данный момент аккаунты группы «{}».",
                group.name
            ),
            "Да, закрыть",
            move |this, _, cx| {
                this.store.update(cx, |store, cx| {
                    let future = store.backend.close_group(group.id);
                    store.fetch(cx, future, |store, closed, cx| {
                        let message = if closed > 0 {
                            format!("Закрыто аккаунтов: {closed}")
                        } else {
                            "Запущенных аккаунтов не было".to_owned()
                        };
                        store.toast(message, false, cx);
                    });
                });
            },
            window,
            cx,
        );
    }

    pub(crate) fn set_group_profile(&mut self, value: String, cx: &mut Context<Self>) {
        let Some(group) = self.active_group(self.store.read(cx)).cloned() else {
            return;
        };
        self.store.update(cx, |store, cx| {
            let message = if value.is_empty() {
                "Профиль группы убран".to_owned()
            } else {
                format!("Профиль группы: {value}")
            };
            let future = store.backend.set_group_profile(group.id, value);
            store.act(cx, future, message);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_prefers_local_then_appends_new() {
        let group = |id: &str| Group {
            id: id.into(),
            ..Group::default()
        };
        let groups = vec![group("a"), group("b"), group("c")];
        let order = vec!["c".to_owned(), "a".to_owned()];
        let ids: Vec<String> = ordered(&groups, &order).into_iter().map(|g| g.id).collect();
        assert_eq!(ids, ["c", "a", "b"]);
    }

    #[test]
    fn toggles_map_to_fields() {
        let mut settings = LaunchSettings::default();
        *toggle_value(&mut settings, "discordRPC") = true;
        assert!(settings.discord_rpc);
        *toggle_value(&mut settings, "debugMode") = true;
        assert!(settings.debug_mode);
    }
}
