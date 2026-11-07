use std::collections::{BTreeSet, HashSet};
use std::time::Duration;

use ac_api::{Account, Group, Rank};
use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, Context, ElementId, Entity, FontWeight,
    Hsla, InteractiveElement, IntoElement, ParentElement, ScrollHandle, StatefulInteractiveElement,
    Styled, StyledText, TextRun, Window, deferred, div, prelude::FluentBuilder, px, relative,
};

use crate::ambient;
use crate::assets::{Icon, icon};
use crate::motion::{self, EASE_OUT, Key};
use crate::platform;
use crate::sound::{self, Sound};
use crate::state::Store;
use crate::theme::{ThemeExt, mix, parse_hex, rem, rgb, rgba, shadow};
use crate::ui::{
    Button, ButtonStyle, Checkbox, IconButton, IconStyle, InputEvent, MenuItem, Spinner, TextInput,
    drop_in, menu_item, menu_separator, menu_surface, scrollbar, with_tooltip,
};
use crate::views::shell::{Popup, Shell};

pub const CARD_MIN_WIDTH: f32 = 350.0;
pub const GRID_GAP: f32 = 12.0;
const SCROLLBAR_WIDTH: f32 = 8.0;
const TELEGRAM_URL: &str = "https://t.me/xpplkn";
const AUTHOR_URL: &str = "https://top.cristalix.gg/profile/Pepel_ok";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortKey {
    Launched,
    Added,
    Expiry,
}

impl SortKey {
    pub const ALL: [(Self, &'static str); 3] = [
        (Self::Launched, "Недавно запущенные"),
        (Self::Added, "Недавно добавленные"),
        (Self::Expiry, "Срок токена"),
    ];

    fn value(self, account: &Account) -> i64 {
        match self {
            Self::Launched => account.last_launched,
            Self::Added => account.first_seen,
            Self::Expiry => account.expires,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub key: SortKey,
    pub ascending: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    pub ranks: BTreeSet<String>,
    pub no_rank: bool,
    pub expired: bool,
}

impl Filters {
    pub fn count(&self, sort: Option<Sort>) -> usize {
        self.ranks.len()
            + usize::from(self.no_rank)
            + usize::from(self.expired)
            + usize::from(sort.is_some())
    }
}

pub fn next_sort(current: Option<Sort>, key: SortKey) -> Option<Sort> {
    match current {
        Some(sort) if sort.key == key && !sort.ascending => Some(Sort {
            key,
            ascending: true,
        }),
        Some(sort) if sort.key == key => None,
        _ => Some(Sort {
            key,
            ascending: false,
        }),
    }
}

pub fn visible<'a>(
    accounts: &'a [Account],
    group: Option<&Group>,
    filters: &Filters,
    sort: Option<Sort>,
    query: &str,
    debug_expired: bool,
    rank: impl Fn(&Account) -> Option<Rank>,
) -> (Vec<&'a Account>, usize) {
    let mut base: Vec<&Account> = accounts
        .iter()
        .filter(|account| group.is_none_or(|group| group.members.contains(&account.uuid)))
        .filter(|account| {
            if filters.ranks.is_empty() && !filters.no_rank {
                return true;
            }
            match rank(account) {
                Some(rank) => filters.ranks.contains(&rank.label),
                None => filters.no_rank,
            }
        })
        .filter(|account| {
            !filters.expired || (account.launchable && (debug_expired || account.expired))
        })
        .collect();
    if let Some(sort) = sort {
        base.sort_by(|a, b| {
            let order = sort.key.value(a).cmp(&sort.key.value(b));
            if sort.ascending {
                order
            } else {
                order.reverse()
            }
        });
    }
    let total = base.len();
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        base.retain(|account| {
            let name = if account.name.is_empty() {
                &account.uuid
            } else {
                &account.name
            };
            name.to_lowercase().contains(&query)
        });
    }
    (base, total)
}

pub fn columns(width: f32) -> usize {
    (((width + GRID_GAP) / (CARD_MIN_WIDTH + GRID_GAP)).floor() as usize).max(1)
}

pub struct Home {
    pub search: Entity<TextInput>,
    pub query: String,
    pub group: Option<String>,
    pub filters: Filters,
    pub sort: Option<Sort>,
    pub debug_expired: bool,
    pub scroll: ScrollHandle,
    pub seen: HashSet<String>,
    pub generation: u64,
    pub first_render: bool,
}

impl Home {
    pub fn new(window: &mut Window, cx: &mut Context<Shell>) -> Self {
        let search = cx.new(|cx| TextInput::new("Поиск по нику…", window, cx));
        cx.subscribe(&search, |shell, input, event, cx| {
            if *event == InputEvent::Changed {
                shell.home.query = input.read(cx).text().to_owned();
                cx.notify();
            }
        })
        .detach();
        Self {
            search,
            query: String::new(),
            group: None,
            filters: Filters::default(),
            sort: None,
            debug_expired: false,
            scroll: ScrollHandle::new(),
            seen: HashSet::new(),
            generation: 0,
            first_render: true,
        }
    }
}

fn rank_labels(store: &Store) -> Vec<Rank> {
    let mut seen = HashSet::new();
    store
        .accounts
        .iter()
        .filter_map(|account| store.rank(account))
        .filter(|rank| seen.insert(rank.label.clone()))
        .collect()
}

impl Shell {
    pub(crate) fn active_group<'a>(&self, store: &'a Store) -> Option<&'a Group> {
        self.home.group.as_deref().and_then(|id| store.group(id))
    }

    pub(crate) fn select_group(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.home.group = id;
        self.home.generation += 1;
        self.home.seen.clear();
        cx.notify();
    }

    pub(crate) fn render_home(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let gutter = self.gutter(window);
        let width = f32::from(window.viewport_size().width);
        let compact = width <= crate::theme::COMPACT_WIDTH;
        let narrow = width <= crate::theme::NARROW_WIDTH;
        let hero_size = if narrow {
            rem(1.2)
        } else if compact {
            rem(1.45)
        } else {
            rem(1.75)
        };
        let store = self.store.read(cx);
        let can_import = store.caps.credential_import;
        let hero =
            div()
                .flex_none()
                .pt(px(if compact { 28.0 } else { 40.0 }))
                .pb(px(if compact { 6.0 } else { 8.0 }))
                .child(
                    div()
                        .mb(px(6.0))
                        .text_size(hero_size)
                        .font_weight(FontWeight::BOLD)
                        .child("Смена аккаунтов"),
                )
                .child(div().text_size(rem(0.9)).text_color(p.muted).child(
                    "Аккаунты подхватываются автоматически, пока приложение работает в фоне.",
                ));

        let search_focused = self.home.search.read(cx).is_focused(window);
        let search = div()
            .id("search")
            .flex()
            .flex_1()
            .min_w_0()
            .when(compact, |s| s.flex_basis(relative(1.0)))
            .items_center()
            .h(px(41.69))
            .px(px(13.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(if search_focused {
                p.line_strong
            } else {
                p.line
            })
            .bg(p.surface)
            .text_size(rem(0.875))
            .child(self.home.search.clone());
        let mut search_box = Some(search);

        let controls = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.0))
            .mt(px(24.0))
            .mb(px(20.0))
            .when(compact, |c| c.flex_wrap())
            .when(!compact, |c| {
                c.child(search_box.take().unwrap_or_else(|| div().id("search-slot")))
            })
            .child(
                Button::new("launcher-btn", "Открыть лаунчер", ButtonStyle::Ghost)
                    .when(compact, |b| b.grow())
                    .on_click(cx.listener(|this, _, _, cx| this.open_launcher(cx))),
            )
            .child(
                Button::new("guest-btn", "Вход без аккаунта", ButtonStyle::Outline)
                    .when(compact, |b| b.grow())
                    .on_click(cx.listener(|this, _, _, cx| this.launch_guest(cx))),
            )
            .when(can_import, |c| {
                c.child(
                    IconButton::new(
                        "refresh-tokens-btn",
                        Key::of("refresh-tokens-btn"),
                        IconStyle::Outline,
                    )
                    .icon(Icon::Refresh, 16.0)
                    .rotate_on_hover(-180.0, Duration::from_millis(550), crate::motion::STANDARD)
                    .rotate_on_press(-180.0, Duration::from_millis(350))
                    .disabled(self.refreshing)
                    .sound(Some(Sound::Confirm))
                    .tooltip("Обновить токены, срок которых истекает в ближайшую неделю")
                    .on_click(cx.listener(|this, _, _, cx| this.refresh_tokens(cx))),
                )
            })
            .children(search_box);

        let group_bar = self.render_group_bar(window, cx);
        let grid = self.render_grid(window, cx);

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .px(px(gutter))
            .child(hero)
            .child(controls)
            .child(group_bar)
            .child(grid)
            .child(self.render_footer(cx))
    }

    fn render_group_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let group = self.active_group(store).cloned();
        let progress = store.group_launch.clone();
        let profiles = store.profiles.profiles.clone();
        let count = self.home.filters.count(self.home.sort);
        let menu_open = self.popup_shown(&Popup::GroupMenu) == Some(None);
        let menu_shown = self.popup_shown(&Popup::GroupMenu).is_some();
        let filter_open = self.popup_shown(&Popup::Filter) == Some(None);
        let filter_shown = self.popup_shown(&Popup::Filter).is_some();
        let caret_key = Key::of("group-caret");
        if motion::is_on(caret_key, cx) != menu_open {
            motion::drive(
                caret_key,
                menu_open,
                Duration::from_millis(160),
                crate::motion::EASE,
                window,
                cx,
            );
        }
        let caret_turn = motion::value(caret_key, cx);
        let trigger_key = Key::of("group-trigger");
        let trigger_hover = motion::value(trigger_key, cx);
        let trigger = div()
            .id("group-trigger")
            .flex()
            .items_center()
            .gap(px(9.0))
            .h(px(40.0))
            .px(px(12.0))
            .min_w(px(210.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(if menu_open {
                p.blue
            } else {
                mix(p.line, p.line_strong, trigger_hover)
            })
            .bg(p.surface_2)
            .text_color(p.text)
            .cursor_pointer()
            .occlude()
            .on_hover(move |hovered, window, cx| motion::hover(trigger_key, *hovered, window, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                sound::play(Sound::Tap);
                this.toggle_popup(Popup::GroupMenu, cx);
            }))
            .child(icon(Icon::Users, px(16.0), p.text))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_size(rem(0.9))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(
                        group
                            .as_ref()
                            .map_or_else(|| "Все аккаунты".to_owned(), |g| g.name.clone()),
                    ),
            )
            .child(icon(Icon::Caret, px(14.0), p.muted).with_transformation(
                gpui::Transformation::rotate(gpui::Radians(std::f32::consts::PI * caret_turn)),
            ));

        let dropdown = div().relative().child(trigger).when(menu_shown, |dd| {
            dd.child(self.render_group_menu(window, cx))
        });

        let actions =
            group.as_ref().map(|group| {
                let launching = progress.active;
                let (label, tip) = match (launching, progress.paused) {
                    (false, _) => ("Запустить все", "Запустить все аккаунты группы поочерёдно"),
                    (true, true) => ("Возобновить", "Возобновить запуск аккаунтов"),
                    (true, false) => ("Остановить", "Остановить запуск аккаунтов"),
                };
                let empty = group.members.is_empty() || !self.group_has_visible(cx);
                let mut launch = Button::new("group-launch", label, ButtonStyle::Primary)
                    .min_width(163.0)
                    .height(40.0)
                    .tooltip(tip)
                    .on_click(cx.listener(|this, _, _, cx| this.group_launch_clicked(cx)));
                if !launching {
                    launch = launch.icon(Icon::PlaySolid, 15.0);
                }
                let mut options: Vec<(String, String)> =
                    vec![(String::new(), "- без профиля -".to_owned())];
                if !group.profile.is_empty() && !profiles.contains(&group.profile) {
                    options.push((group.profile.clone(), group.profile.clone()));
                }
                options.extend(profiles.iter().map(|name| (name.clone(), name.clone())));
                let current = group.profile.clone();
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(10.0))
                    .child(launch)
                    .when(!empty, |row| {
                        row.child(
                            Button::new("group-add", "Добавить аккаунты", ButtonStyle::Tinted)
                                .height(40.0)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_add_members(None, window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("group-ram", "Настройки запуска", ButtonStyle::Tinted)
                            .height(40.0)
                            .tooltip("Настройки запуска для аккаунтов группы")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_launch_settings(window, cx)
                            })),
                    )
                    .child(self.render_group_profile(options, current, window, cx))
                    .child(
                        IconButton::new(
                            "group-close-all",
                            Key::of("group-close-all"),
                            IconStyle::Danger,
                        )
                        .icon(Icon::CloseCircle, 16.0)
                        .tooltip("Закрыть все запущенные аккаунты группы")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.confirm_close_group(window, cx)),
                        ),
                    )
            });

        let filter_key = Key::of("filter-trigger");
        let filter = div()
            .relative()
            .ml_auto()
            .child(
                IconButton::new("filter-trigger", filter_key, IconStyle::Filter)
                    .icon(Icon::Filter, 18.0)
                    .active(filter_open)
                    .tooltip("Фильтры и сортировка")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Filter, cx))),
            )
            .when(count > 0, |f| {
                f.child(
                    div()
                        .absolute()
                        .top(px(-6.0))
                        .right(px(-6.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .min_w(px(18.0))
                        .h(px(18.0))
                        .px(px(5.0))
                        .rounded_full()
                        .bg(p.blue)
                        .text_color(p.white)
                        .text_size(rem(0.68))
                        .font_weight(FontWeight::BOLD)
                        .child(count.to_string()),
                )
            })
            .when(filter_shown, |f| {
                f.child(self.render_filter_panel(window, cx))
            });

        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap_y(px(12.0))
            .mb(px(16.0))
            .child(dropdown.mr(px(12.0)))
            .children(actions)
            .child(filter)
    }

    fn group_has_visible(&self, cx: &App) -> bool {
        let store = self.store.read(cx);
        let Some(group) = self.active_group(store) else {
            return true;
        };
        store
            .accounts
            .iter()
            .any(|account| group.members.contains(&account.uuid))
    }

    fn render_group_profile(
        &mut self,
        options: Vec<(String, String)>,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let key = Key::of("group-profile-row");
        let hover = motion::value(key, cx);
        let dropdown = self.render_dropdown(
            "group-profile",
            options,
            current,
            true,
            window,
            cx,
            |this, value, _, cx| this.set_group_profile(value, cx),
        );
        let row = div()
            .id("group-profile-row")
            .flex()
            .items_center()
            .gap(px(8.0))
            .h(px(40.0))
            .min_w_0()
            .max_w(px(260.0))
            .pl(px(12.0))
            .pr(px(6.0))
            .mr(px(2.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(mix(p.line, p.line_strong, hover))
            .bg(p.surface)
            .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
            .child(icon(Icon::GroupProfile, px(13.0), p.muted))
            .child(
                div()
                    .text_size(rem(0.72))
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.muted)
                    .child("Профиль"),
            )
            .child(div().min_w(px(148.0)).flex_1().child(dropdown));
        with_tooltip(row, key, "Профиль настроек для группы".into())
    }

    fn render_group_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let selected = self.home.group.clone();
        let groups: Vec<Group> = store.groups.clone();
        let viewport = f32::from(window.viewport_size().height);
        let max_height = (viewport - 320.0 - 18.0).clamp(120.0, 340.0);
        let mut menu = menu_surface(p.surface_2, 11.0, 7.0, cx)
            .gap(px(3.0))
            .min_w(px(260.0))
            .max_h(px(max_height))
            .text_size(rem(0.86));
        let item = |id: ElementId, active: bool| {
            menu_item(
                id,
                if active {
                    MenuItem::Active
                } else {
                    MenuItem::Plain
                },
                &p,
            )
            .px(px(10.0))
            .py(px(9.0))
        };
        menu = menu.child(
            item("group-all".into(), selected.is_none())
                .on_click(cx.listener(|this, _, _, cx| {
                    sound::play(Sound::Tap);
                    this.close_popup(cx);
                    this.select_group(None, cx);
                }))
                .child(div().flex_1().child("Все аккаунты")),
        );
        for group in groups {
            let id = group.id.clone();
            let active = selected.as_deref() == Some(group.id.as_str());
            menu = menu.child(
                item(
                    ElementId::Name(format!("group-{}", group.id).into()),
                    active,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    sound::play(Sound::Tap);
                    this.close_popup(cx);
                    this.select_group(Some(id.clone()), cx);
                }))
                .when(group.pinned, |row| {
                    row.child(icon(Icon::StarSolid, px(13.0), p.yellow))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .child(group.name.clone()),
                )
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(1.0))
                        .rounded(px(20.0))
                        .bg(p.count_bg)
                        .text_size(rem(0.72))
                        .text_color(p.muted)
                        .child(group.members.len().to_string()),
                ),
            );
        }
        menu = menu
            .child(menu_separator(&p, 4.0, 6.0))
            .child(
                menu_item("group-create", MenuItem::Action, &p)
                    .px(px(10.0))
                    .py(px(9.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        sound::play(Sound::Tap);
                        this.close_popup(cx);
                        this.create_group_flow(window, cx);
                    }))
                    .group("group-create")
                    .child(
                        icon(Icon::Plus, px(14.0), p.muted)
                            .group_hover("group-create", |glyph| glyph.text_color(p.text)),
                    )
                    .child("Создать группу"),
            )
            .child(
                menu_item("group-manage", MenuItem::Action, &p)
                    .px(px(10.0))
                    .py(px(9.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        sound::play(Sound::Tap);
                        this.close_popup(cx);
                        this.open_groups(window, cx);
                    }))
                    .group("group-manage")
                    .child(
                        icon(Icon::Gear, px(14.0), p.muted)
                            .group_hover("group-manage", |glyph| glyph.text_color(p.text)),
                    )
                    .child("Управление группами"),
            );
        deferred(div().absolute().top(px(46.0)).left_0().child(drop_in(
            "group-menu-in",
            menu.id("group-menu").overflow_y_scroll().on_mouse_down_out(
                cx.listener(|this, _, _, cx| this.dismiss_popup(Popup::GroupMenu, cx)),
            ),
            self.popup_shown(&Popup::GroupMenu).flatten(),
        )))
        .with_priority(1)
    }

    fn render_filter_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let ranks = rank_labels(store);
        let filters = self.home.filters.clone();
        let sort = self.home.sort;
        let title = |text: &'static str| {
            div()
                .px(px(6.0))
                .pt(px(4.0))
                .pb(px(6.0))
                .text_size(rem(0.66))
                .font_weight(FontWeight::BOLD)
                .text_color(p.faint)
                .child(crate::ui::spaced(&text.to_uppercase(), 0.5))
        };
        let row = |id: ElementId| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(10.0))
                .p(px(8.0))
                .rounded(px(8.0))
                .text_size(rem(0.83))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text)
                .cursor_pointer()
                .hover(|s| s.bg(p.wash))
        };
        let mut panel = menu_surface(p.surface_2, 12.0, 10.0, cx)
            .w(px(262.0))
            .child(title("Фильтр по группе"));
        if ranks.is_empty() {
            panel = panel.child(
                div()
                    .px(px(6.0))
                    .pt(px(4.0))
                    .pb(px(8.0))
                    .text_size(rem(0.76))
                    .text_color(p.muted)
                    .child("Группы аккаунтов ещё не загружены"),
            );
        }
        for rank in ranks {
            let on = filters.ranks.contains(&rank.label);
            let color = parse_hex(&rank.color);
            let label = rank.label.clone();
            panel = panel.child(
                row(ElementId::Name(
                    format!("filter-rank-{}", rank.label).into(),
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    sound::play(Sound::Tap);
                    if !this.home.filters.ranks.remove(&label) {
                        this.home.filters.ranks.insert(label.clone());
                    }
                    cx.notify();
                }))
                .child(Checkbox::new(on).color(color))
                .child(
                    div()
                        .flex_1()
                        .text_color(color.unwrap_or(p.text))
                        .child(rank.label.clone()),
                ),
            );
        }
        panel = panel
            .child(
                row("filter-norank".into())
                    .on_click(cx.listener(|this, _, _, cx| {
                        sound::play(Sound::Tap);
                        this.home.filters.no_rank = !this.home.filters.no_rank;
                        cx.notify();
                    }))
                    .child(Checkbox::new(filters.no_rank))
                    .child(div().flex_1().child("Без роли")),
            )
            .child(
                row("filter-expired".into())
                    .on_click(cx.listener(|this, _, _, cx| {
                        sound::play(Sound::Tap);
                        this.home.filters.expired = !this.home.filters.expired;
                        cx.notify();
                    }))
                    .child(Checkbox::new(filters.expired))
                    .child(div().flex_1().child("Истёкшие токены")),
            )
            .child(menu_separator(&p, 4.0, 8.0))
            .child(title("Сортировка"));
        for (key, label) in SortKey::ALL {
            let active = sort.filter(|s| s.key == key);
            let arrow_key = Key::new("sort-arrow", label);
            let down = matches!(
                active,
                Some(Sort {
                    ascending: false,
                    ..
                })
            );
            if motion::is_on(arrow_key, cx) != down {
                motion::drive(
                    arrow_key,
                    down,
                    Duration::from_millis(160),
                    motion::EASE,
                    window,
                    cx,
                );
            }
            let turn = std::f32::consts::PI * motion::value(arrow_key, cx);
            panel = panel.child(
                row(ElementId::Name(format!("sort-{label}").into()))
                    .when(active.is_some(), |r| {
                        r.bg(rgba(0x3b82e8, 0.12)).text_color(p.active_text)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        sound::play(Sound::Tap);
                        this.home.sort = next_sort(this.home.sort, key);
                        cx.notify();
                    }))
                    .child(div().flex_1().child(label))
                    .child(
                        icon(
                            Icon::ArrowUp,
                            px(14.0),
                            if active.is_some() { p.blue } else { p.faint },
                        )
                        .with_transformation(gpui::Transformation::rotate(gpui::Radians(turn))),
                    ),
            );
        }
        let reset_key = Key::of("filter-reset");
        let reset_hover = motion::value(reset_key, cx);
        panel = panel.child(
            div()
                .id("filter-reset")
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .mt(px(6.0))
                .p(px(8.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(mix(p.line, p.line_strong, reset_hover))
                .text_color(mix(p.muted, p.text, reset_hover))
                .text_size(rem(0.8))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .on_hover(move |hovered, window, cx| motion::hover(reset_key, *hovered, window, cx))
                .on_click(cx.listener(|this, _, _, cx| {
                    sound::play(Sound::Tap);
                    this.home.filters = Filters::default();
                    this.home.sort = None;
                    cx.notify();
                }))
                .child(icon(
                    Icon::Reset,
                    px(13.0),
                    mix(p.muted, p.text, reset_hover),
                ))
                .child("Сбросить всё"),
        );
        deferred(div().absolute().top(px(46.0)).right_0().child(drop_in(
            "filter-panel-in",
            panel.id("filter-panel").on_mouse_down_out(
                cx.listener(|this, _, _, cx| this.dismiss_popup(Popup::Filter, cx)),
            ),
            self.popup_shown(&Popup::Filter).flatten(),
        )))
        .with_priority(1)
    }

    fn render_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette();
        let gutter = self.gutter(window);
        let store = self.store.read(cx);
        let group = self.active_group(store);
        let has_group = group.is_some();
        let no_accounts = store.accounts_loaded && store.accounts.is_empty();
        let (list, total) = visible(
            &store.accounts,
            group,
            &self.home.filters,
            self.home.sort,
            &self.home.query,
            self.home.debug_expired,
            |account| store.rank(account),
        );
        let uuids: Vec<String> = list.iter().map(|a| a.uuid.clone()).collect();
        let accounts: Vec<Account> = list.into_iter().cloned().collect();
        let max_offset = f32::from(self.home.scroll.max_offset().height);
        let overflows = max_offset > 0.5;
        let width = f32::from(window.viewport_size().width)
            - gutter * 2.0
            - if overflows { SCROLLBAR_WIDTH } else { 0.0 };
        let cols = columns(width);
        let card_width = (width - GRID_GAP * (cols as f32 - 1.0)) / cols as f32;
        let first = self.home.first_render && !accounts.is_empty();
        let generation = self.home.generation;
        let replay = generation > 0 && self.home.seen.is_empty() && !first;
        self.home.seen = uuids.iter().cloned().collect();
        if !accounts.is_empty() {
            self.home.first_render = false;
        }

        let mut rows: Vec<AnyElement> = Vec::new();
        for (row_index, chunk) in accounts.chunks(cols).enumerate() {
            let mut row = div().flex().gap(px(GRID_GAP)).w_full();
            for (col, account) in chunk.iter().enumerate() {
                let index = row_index * cols + col;
                let card = self
                    .render_card(account, card_width, window, cx)
                    .into_any_element();
                let delay = if first {
                    Duration::from_millis((index as u64 * 24).min(260))
                } else if replay {
                    Duration::from_millis((index as u64 * 26).min(300))
                } else {
                    Duration::ZERO
                };
                let cell = enter(
                    div().flex_1().min_w_0().child(card),
                    &account.uuid,
                    generation,
                    delay,
                );
                row = row.child(cell);
            }
            for _ in chunk.len()..cols {
                row = row.child(div().flex_1().min_w_0());
            }
            rows.push(row.into_any_element());
        }

        let empty_group = has_group && total == 0;
        let body = if no_accounts && !has_group {
            div()
                .py(px(48.0))
                .px(px(18.0))
                .flex()
                .justify_center()
                .text_size(rem(0.9))
                .text_color(p.muted)
                .child(
                    "Пока пусто. Зайди на любой аккаунт через лаунчер - токен сохранится сюда сам.",
                )
                .into_any_element()
        } else if empty_group {
            let key = Key::of("group-add-center");
            let hover = motion::value(key, cx);
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(22.0))
                .pt(px(104.0))
                .pb(px(48.0))
                .child(
                    div()
                        .text_size(rem(0.9))
                        .text_color(p.muted)
                        .child("В этой группе пока нет аккаунтов"),
                )
                .child(with_tooltip(
                    div()
                        .id("group-add-center")
                        .relative()
                        .top(px(-2.0 * hover))
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(84.0))
                        .h(px(68.0))
                        .rounded(px(13.0))
                        .border(px(1.5))
                        .border_color(mix(p.line_strong, p.blue, hover))
                        .bg(mix(p.surface_2, rgba(0x3b82e8, 0.09), hover))
                        .shadow(shadow(
                            0.0,
                            5.0 + 5.0 * hover,
                            16.0 + 14.0 * hover,
                            0.0,
                            mix(rgba(0x000000, 0.35), rgba(0x3b82e8, 0.34), hover),
                        ))
                        .cursor_pointer()
                        .on_hover(move |hovered, window, cx| {
                            motion::hover(key, *hovered, window, cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            sound::play(Sound::Tap);
                            this.open_add_members(None, window, cx);
                        }))
                        .child(icon(Icon::Plus, px(28.0), mix(p.muted, p.blue, hover))),
                    key,
                    "Добавить аккаунты".into(),
                ))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(GRID_GAP))
                .mb(px(8.0))
                .children(rows)
                .into_any_element()
        };

        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("acc-scroll")
                    .flex()
                    .flex_col()
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.home.scroll)
                    .on_scroll_wheel(cx.listener(|this, _, _, cx| this.close_card_menu(cx)))
                    .pt(px(8.0))
                    .pb(px(10.0))
                    .when(overflows, |s| s.pr(px(SCROLLBAR_WIDTH)))
                    .child(body),
            )
            .children(scrollbar(
                "acc-scrollbar",
                &self.home.scroll,
                SCROLLBAR_WIDTH,
                0.0,
                cx,
            ))
            .into_any_element()
    }

    fn render_footer(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = cx.palette();
        let shimmer = ambient::cycle(Duration::from_secs(7), cx).unwrap_or(0.0);
        let heart = ambient::cycle(Duration::from_millis(2600), cx).unwrap_or(0.0);
        let key = Key::of("footer-name");
        let hover = motion::value(key, cx);
        let name = "xpepelok";
        let stops = [rgb(0x7aa8ff), rgb(0xa78bfa), rgb(0x7ee7d6), rgb(0x7aa8ff)];
        let runs: Vec<TextRun> = name
            .char_indices()
            .map(|(index, ch)| {
                let x = (index as f32 + 0.5) / name.len() as f32 / 3.0 + shimmer;
                TextRun {
                    len: ch.len_utf8(),
                    font: gpui::Font {
                        weight: FontWeight::BOLD,
                        ..gpui::font(crate::theme::FONT_FAMILY)
                    },
                    color: mix(gradient(&stops, x.fract()), p.white, 0.25 * hover),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }
            })
            .collect();
        let pulse = motion::keyframes(
            &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
            heart,
            motion::EASE_IN_OUT,
        );
        let glow = motion::keyframes(
            &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
            ambient::cycle(Duration::from_millis(3400), cx).unwrap_or(0.0),
            motion::EASE_IN_OUT,
        );
        div()
            .flex_none()
            .flex()
            .justify_center()
            .items_center()
            .pt(px(30.0))
            .pb(px(20.0))
            .text_size(rem(0.78))
            .text_color(p.faint)
            .child("with love from\u{a0}")
            .child(
                div()
                    .id("footer-name")
                    .rounded_full()
                    .shadow(shadow(
                        0.0,
                        0.0,
                        3.0 + 4.0 * glow,
                        0.0,
                        mix(rgba(0x7aa8ff, 0.3), rgba(0xa78bfa, 0.5), glow),
                    ))
                    .cursor_pointer()
                    .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
                    .on_click(|_, _, _| {
                        sound::play(Sound::Tap);
                        platform::open_url(AUTHOR_URL);
                    })
                    .child(StyledText::new(name).with_runs(runs)),
            )
            .child("\u{a0}")
            .child(
                div()
                    .relative()
                    .w(px(10.0))
                    .h(px(19.0))
                    .child(
                        div()
                            .absolute()
                            .left(px(5.0 - 1.0))
                            .top(px(9.5 - 1.0))
                            .size(px(2.0))
                            .rounded_full()
                            .shadow(shadow(
                                0.0,
                                0.0,
                                4.0 + 6.0 * pulse,
                                2.0,
                                rgba(0xe0564f, 0.35 + 0.35 * pulse),
                            )),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(-10.0))
                            .w(px(30.0))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(0xe0564f))
                            .text_size(rem(0.78 * (1.0 + 0.14 * pulse)))
                            .child("♥"),
                    ),
            )
    }

    pub(crate) fn render_fabs(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let import = store.import.clone();
        let can_import = store.caps.credential_import;
        let queue_len = store.queue.len();
        let peak = store.queue_peak.max(queue_len).max(1);
        let tg_key = Key::of("tg-fab");
        let tg_hover = motion::value(tg_key, cx);
        let telegram = with_tooltip(
            div()
                .id("tg-fab")
                .absolute()
                .left(px(20.0))
                .bottom(px(20.0))
                .size(px(36.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .opacity(0.7 + 0.3 * tg_hover)
                .occlude()
                .on_hover(move |hovered, window, cx| {
                    motion::drive(
                        tg_key,
                        *hovered,
                        Duration::from_millis(320),
                        motion::EASE,
                        window,
                        cx,
                    )
                })
                .on_click(|_, _, _| {
                    sound::play(Sound::Tap);
                    platform::open_url(TELEGRAM_URL);
                })
                .child(
                    div()
                        .relative()
                        .top(px(-tg_hover))
                        .rounded_full()
                        .shadow(shadow(
                            0.0,
                            0.0,
                            13.0 * tg_hover,
                            0.0,
                            rgba(0x3b82e8, 0.5 * tg_hover),
                        ))
                        .child(icon(
                            Icon::Telegram,
                            px(27.0),
                            mix(rgb(0x7d8798), p.blue, tg_hover),
                        )),
                ),
            tg_key,
            "Dev-блог разработчика".into(),
        );

        let fab_key = Key::of("cred-btn");
        let fab_hover = motion::value(fab_key, cx);
        let loading = import.running;
        let fab_tone = if loading { 1.0 } else { fab_hover };
        let import_fab = can_import.then(|| {
            with_tooltip(
                div()
                    .id("cred-btn")
                    .absolute()
                    .right(px(22.0))
                    .bottom(px(22.0 + 2.0 * fab_hover))
                    .size(px(46.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(13.0))
                    .border_1()
                    .border_color(mix(p.line_strong, p.blue, fab_tone))
                    .bg(mix(p.surface, p.surface_2, fab_hover))
                    .shadow(shadow(
                        0.0,
                        8.0 - 3.0 * fab_hover,
                        22.0 - 4.0 * fab_hover,
                        0.0,
                        mix(rgba(0x000000, 0.32), rgba(0x3b82e8, 0.28), fab_hover),
                    ))
                    .cursor_pointer()
                    .occlude()
                    .on_hover(move |hovered, window, cx| {
                        motion::hover(fab_key, *hovered, window, cx)
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        sound::play(Sound::Confirm);
                        this.open_import(window, cx);
                    }))
                    .map(|fab| {
                        if loading {
                            fab.child(Spinner::new("fab-spin", 20.0))
                        } else {
                            fab.child(icon(
                                Icon::ImportUser,
                                px(22.0),
                                mix(p.text, p.blue, fab_hover),
                            ))
                        }
                    })
                    .when(loading && import.total > 0, |fab| {
                        fab.child(
                            div()
                                .absolute()
                                .top(px(-5.0))
                                .right(px(-5.0))
                                .min_w(px(17.0))
                                .h(px(17.0))
                                .px(px(4.0))
                                .rounded(px(9.0))
                                .bg(p.blue)
                                .text_color(p.white)
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .line_height(px(17.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .whitespace_nowrap()
                                .shadow(shadow(0.0, 2.0, 6.0, 0.0, rgba(0x000000, 0.3)))
                                .child(format!("{}/{}", import.done, import.total)),
                        )
                    }),
                fab_key,
                "Импорт аккаунтов".into(),
            )
        });

        let queue = (queue_len > 0).then(|| {
            let key = Key::of("queue-btn");
            let hover = motion::value(key, cx);
            let fill = ((peak - queue_len) as f32 / peak as f32).clamp(0.0, 1.0);
            let open = self.popup_shown(&Popup::Queue).is_some();
            div()
                .absolute()
                .right(px(22.0))
                .bottom(px(78.0))
                .child(with_tooltip(
                    div()
                        .id("queue-btn")
                        .relative()
                        .top(px(-2.0 * hover))
                        .size(px(46.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(13.0))
                        .border_1()
                        .border_color(mix(p.line_strong, p.blue, hover))
                        .bg(mix(p.surface, p.surface_2, hover))
                        .shadow(shadow(0.0, 8.0, 22.0, 0.0, rgba(0x000000, 0.32)))
                        .cursor_pointer()
                        .occlude()
                        .on_hover(move |hovered, window, cx| {
                            motion::hover(key, *hovered, window, cx)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            sound::play(Sound::Tap);
                            this.toggle_popup(Popup::Queue, cx);
                        }))
                        .child(ring(fill, p.blue, p.line_strong))
                        .child(
                            div()
                                .text_size(rem(0.86))
                                .font_weight(FontWeight::BOLD)
                                .text_color(mix(p.text, p.blue, hover))
                                .child(queue_len.to_string()),
                        ),
                    key,
                    "Очередь запускаемых аккаунтов".into(),
                ))
                .when(open, |q| q.child(self.render_queue_menu(cx)))
        });

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(telegram)
            .children(queue)
            .children(import_fab)
    }

    fn render_queue_menu(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let entries = store.queue.clone();
        let ranks: Vec<Option<Rank>> = entries
            .iter()
            .map(|entry| store.account(&entry.uuid).and_then(|a| store.rank(a)))
            .collect();
        let mut menu = menu_surface(p.surface, 12.0, 8.0, cx)
            .gap(px(4.0))
            .min_w(px(234.0))
            .max_w(px(300.0))
            .max_h(px(340.0))
            .child(
                div()
                    .px(px(5.0))
                    .pt(px(2.0))
                    .pb(px(5.0))
                    .text_size(rem(0.64))
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.muted)
                    .child(crate::ui::spaced("ОЧЕРЕДЬ ЗАПУСКА", 0.4)),
            );
        for (entry, rank) in entries.into_iter().zip(ranks) {
            let launching = entry.state == ac_api::QueueState::Launching;
            let name = if entry.name.is_empty() {
                entry.uuid.chars().take(8).collect()
            } else {
                entry.name.clone()
            };
            menu = menu.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(7.0))
                    .rounded(px(9.0))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.surface_2)
                    .child(crate::ui::Avatar::new(
                        &self.store,
                        entry.uuid.clone(),
                        22.0,
                        5.0,
                    ))
                    .children(
                        rank.map(|rank| crate::ui::Chip::new(rank.label, rank.color).compact()),
                    )
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
                            .text_size(rem(0.62))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if launching { p.blue } else { p.muted })
                            .child(crate::ui::spaced(
                                if launching {
                                    "ЗАПУСК"
                                } else {
                                    "В ОЧЕРЕДИ"
                                },
                                0.3,
                            )),
                    ),
            );
        }
        deferred(div().absolute().bottom(px(54.0)).right_0().child(drop_in(
            "queue-menu-in",
            menu.id("queue-menu").overflow_y_scroll().on_mouse_down_out(
                cx.listener(|this, _, _, cx| this.dismiss_popup(Popup::Queue, cx)),
            ),
            self.popup_shown(&Popup::Queue).flatten(),
        )))
        .with_priority(1)
    }
}

fn gradient(stops: &[Hsla], t: f32) -> Hsla {
    let segments = (stops.len() - 1) as f32;
    let at = t.clamp(0.0, 1.0) * segments;
    let index = (at.floor() as usize).min(stops.len() - 2);
    mix(stops[index], stops[index + 1], at - index as f32)
}

fn ring(fill: f32, on: Hsla, off: Hsla) -> impl IntoElement + use<> {
    gpui::canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let inset = 5.0;
            let center = bounds.center();
            let radius = f32::from(bounds.size.width) / 2.0 - inset - 1.5;
            let arc = |from: f32, to: f32, color: Hsla, window: &mut Window| {
                if to - from <= 0.0001 {
                    return;
                }
                let mut builder = gpui::PathBuilder::stroke(px(3.0));
                let steps = ((to - from) * 64.0).ceil().max(2.0) as usize;
                for step in 0..=steps {
                    let angle = (from + (to - from) * step as f32 / steps as f32)
                        * std::f32::consts::TAU
                        - std::f32::consts::FRAC_PI_2;
                    let point = gpui::point(
                        center.x + px(radius * angle.cos()),
                        center.y + px(radius * angle.sin()),
                    );
                    if step == 0 {
                        builder.move_to(point);
                    } else {
                        builder.line_to(point);
                    }
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            };
            arc(fill, 1.0, off, window);
            arc(0.0, fill, on, window);
        },
    )
    .absolute()
    .size_full()
}

fn enter(
    cell: gpui::Div,
    uuid: &str,
    generation: u64,
    delay: Duration,
) -> impl IntoElement + use<> {
    let id = ElementId::Name(format!("enter-{generation}-{uuid}").into());
    let rise =
        Animation::new(Duration::from_millis(320)).with_easing(crate::motion::eased(EASE_OUT));
    if delay.is_zero() {
        cell.with_animations(id, vec![rise], |cell, _, t| {
            cell.opacity(t)
                .mt(px(10.0 * (1.0 - t)))
                .mb(px(-10.0 * (1.0 - t)))
        })
    } else {
        cell.with_animations(id, vec![Animation::new(delay), rise], |cell, phase, t| {
            let t = if phase == 0 { 0.0 } else { t };
            cell.opacity(t)
                .mt(px(10.0 * (1.0 - t)))
                .mb(px(-10.0 * (1.0 - t)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(uuid: &str, name: &str, launched: i64) -> Account {
        Account {
            uuid: uuid.into(),
            name: name.into(),
            launchable: true,
            last_launched: launched,
            ..Account::default()
        }
    }

    #[test]
    fn columns_follow_auto_fill() {
        assert_eq!(columns(300.0), 1);
        assert_eq!(columns(712.0), 2);
        assert_eq!(columns(1400.0), 3);
        assert_eq!(columns(1448.0), 4);
    }

    #[test]
    fn sort_cycles_desc_asc_off() {
        let first = next_sort(None, SortKey::Added);
        assert_eq!(
            first,
            Some(Sort {
                key: SortKey::Added,
                ascending: false
            })
        );
        let second = next_sort(first, SortKey::Added);
        assert_eq!(
            second,
            Some(Sort {
                key: SortKey::Added,
                ascending: true
            })
        );
        assert_eq!(next_sort(second, SortKey::Added), None);
        assert_eq!(
            next_sort(second, SortKey::Expiry).unwrap().key,
            SortKey::Expiry
        );
    }

    #[test]
    fn filtering_searching_and_sorting() {
        let accounts = vec![
            account("1", "Alpha", 10),
            account("2", "beta", 30),
            account("3", "Gamma", 20),
        ];
        let (all, total) = visible(
            &accounts,
            None,
            &Filters::default(),
            None,
            "",
            false,
            |_| None,
        );
        assert_eq!((all.len(), total), (3, 3));
        let sort = Some(Sort {
            key: SortKey::Launched,
            ascending: false,
        });
        let (sorted, _) = visible(
            &accounts,
            None,
            &Filters::default(),
            sort,
            "",
            false,
            |_| None,
        );
        assert_eq!(
            sorted.iter().map(|a| a.uuid.as_str()).collect::<Vec<_>>(),
            ["2", "3", "1"]
        );
        let (found, total) = visible(
            &accounts,
            None,
            &Filters::default(),
            None,
            "BET",
            false,
            |_| None,
        );
        assert_eq!((found.len(), total), (1, 3));
        let group = Group {
            members: vec!["3".into()],
            ..Group::default()
        };
        let (members, _) = visible(
            &accounts,
            Some(&group),
            &Filters::default(),
            None,
            "",
            false,
            |_| None,
        );
        assert_eq!(members.len(), 1);
        let filters = Filters {
            no_rank: true,
            ..Filters::default()
        };
        let (no_rank, _) = visible(&accounts, None, &filters, None, "", false, |a| {
            (a.uuid == "1").then(|| Rank {
                label: "VIP".into(),
                color: "#fff".into(),
            })
        });
        assert_eq!(no_rank.len(), 2);
    }
}
