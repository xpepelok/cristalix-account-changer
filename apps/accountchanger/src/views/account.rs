use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use ac_api::{Account, RAM_DEFAULT, RAM_MAX, RAM_MIN, RAM_STEP, TextureKind, TokenState, snap_ram};
use futures::channel::oneshot;
use gpui::{
    AnyElement, AppContext, ClipboardItem, Context, ElementId, Entity, Focusable, FontWeight, Hsla,
    InteractiveElement, IntoElement, MouseButton, ObjectFit, ParentElement, Render, RenderImage,
    ScrollHandle, StatefulInteractiveElement, Styled, StyledImage, Task, Window, canvas, div, img,
    prelude::FluentBuilder, px, relative,
};

use crate::assets::{Icon, icon};
use crate::format::{date_time, iso_date_time, subscription_label};
use crate::motion::{self, Key};
use crate::platform;
use crate::skin::{self, STAGE_FRAME, STAGE_IDLE, Stage, Texture};
use crate::sound::{self, Sound};
use crate::theme::{Palette, ThemeExt, alpha, mix, parse_hex, rem, rgb, rgba};
use crate::ui::{
    Button, ButtonStyle, IconButton, IconStyle, InputEvent, ModalFrame, ScrollArea, Slider,
    TextInput, spaced, with_tooltip,
};
use crate::views::shell::Shell;
use crate::views::widgets::field_label;

const COPIED_FOR: Duration = Duration::from_millis(1200);

fn glow() -> Arc<RenderImage> {
    static GLOW: OnceLock<Arc<RenderImage>> = OnceLock::new();
    GLOW.get_or_init(|| skin::to_render_image(skin::stage_glow(210, 420)))
        .clone()
}

struct StageDrag;

struct StageGhost;

impl Render for StageGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub struct AccountView {
    pub uuid: String,
    label: Entity<TextInput>,
    ram_input: Entity<TextInput>,
    ram_draft: Option<u32>,
    cape: Option<Vec<u8>>,
    pub stage: Stage,
    _stage_task: Task<()>,
    copied_at: Option<Instant>,
    stats_scroll: ScrollHandle,
}

fn ram_of(account: &Account) -> u32 {
    if account.launch.auto_ram() {
        RAM_DEFAULT
    } else {
        account.launch.ram
    }
}

struct Tile {
    value: String,
    label: &'static str,
    tone: Option<Hsla>,
}

fn tiles(account: &Account, shell: &Shell, cx: &gpui::App, p: &Palette) -> Vec<Tile> {
    let store = shell.store.read(cx);
    let info = store.player(&account.name);
    let status = store.online_status(account);
    let token = account.token_state();
    let mut tiles = Vec::new();
    if let Some(status) = &status {
        tiles.push(Tile {
            value: status.text.clone(),
            label: "статус",
            tone: status.on.then_some(p.green),
        });
    }
    let (token_text, token_tone) = match token {
        TokenState::Ready => ("готов", Some(p.green)),
        TokenState::Expired => ("истёк", Some(p.yellow)),
        TokenState::Missing => ("нет токена", None),
    };
    tiles.push(Tile {
        value: token_text.to_owned(),
        label: "статус токена",
        tone: token_tone,
    });
    tiles.push(Tile {
        value: if account.running { "в игре" } else { "-" }.to_owned(),
        label: "клиент",
        tone: account.running.then_some(p.green),
    });
    tiles.push(Tile {
        value: date_time(account.expires),
        label: "токен до",
        tone: Some(p.blue),
    });
    tiles.push(Tile {
        value: date_time(account.last_launched),
        label: "последний запуск",
        tone: None,
    });
    if let Some(info) = info {
        if !info.subscription.is_empty() {
            tiles.push(Tile {
                value: subscription_label(&info.subscription),
                label: "подписка",
                tone: Some(p.blue),
            });
        }
        if info.likes != 0 || info.views != 0 {
            tiles.push(Tile {
                value: format!("{} / {}", info.likes, info.views),
                label: "лайки / просмотры",
                tone: None,
            });
        }
        if !info.registered_at.is_empty() {
            tiles.push(Tile {
                value: iso_date_time(&info.registered_at),
                label: "регистрация",
                tone: None,
            });
        }
        if !info.last_seen.is_empty() && !status.as_ref().is_some_and(|s| s.on) {
            tiles.push(Tile {
                value: iso_date_time(&info.last_seen),
                label: "был онлайн",
                tone: None,
            });
        }
    }
    tiles
}

pub fn stat_tile(value: impl IntoElement, label: &str, cx: &gpui::App) -> gpui::Div {
    let p = cx.palette();
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .min_w_0()
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_2)
        .child(value)
        .child(
            div()
                .text_size(rem(0.62))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.faint)
                .child(label.to_uppercase()),
        )
}

impl AccountView {
    pub fn flush_label(&self, shell: &mut Shell, cx: &mut Context<Shell>) {
        let value = self.label.read(cx).text().trim().to_owned();
        let Some(account) = shell.store.read(cx).account(&self.uuid).cloned() else {
            return;
        };
        if value == account.label {
            return;
        }
        shell.store.update(cx, |store, cx| {
            let future = store.backend.set_label(account.uuid.clone(), value);
            store.act(cx, future, "");
        });
    }

    pub fn render(
        &mut self,
        shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let Some(account) = shell.store.read(cx).account(&self.uuid).cloned() else {
            return div().into_any_element();
        };
        let store = shell.store.read(cx);
        let ranks = store
            .player(&account.name)
            .map(|info| info.ranks())
            .unwrap_or_default();
        let profiles = store.profiles.profiles.clone();
        let has_skin = matches!(store.skin_texture(&account.uuid), Some(Texture::Ready(_)));
        let has_cape = self.cape.is_some();
        let frame = self.stage.frame.clone();
        let shell_handle = cx.entity().downgrade();
        let tiles = tiles(&account, shell, cx, &p);
        let auto_ram = account.launch.auto_ram();
        let ram = self.ram_draft.unwrap_or_else(|| ram_of(&account));
        let copied = self.copied_at.is_some_and(|at| at.elapsed() < COPIED_FOR);
        let label_focused = self.label.read(cx).is_focused(window);
        let ram_focused = self.ram_input.read(cx).is_focused(window);
        let uuid = account.uuid.clone();

        let stage = div()
            .relative()
            .flex_1()
            .w_full()
            .min_h(px(270.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(p.line)
            .bg(p.graphite)
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .child(
                img(glow())
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                canvas(
                    move |bounds, _, cx| {
                        if let Some(shell) = shell_handle.upgrade() {
                            shell.update(cx, |shell, _| {
                                if let Some(view) = shell.overlay::<AccountView>() {
                                    view.stage.size = (
                                        f32::from(bounds.size.width),
                                        f32::from(bounds.size.height),
                                    );
                                }
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .children(frame.map(|image| {
                img(image)
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill)
            }))
            .id("skin-stage")
            .cursor_grab()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    if let Some(view) = this.overlay::<AccountView>() {
                        view.stage.grab =
                            Some((f32::from(event.position.x), f32::from(event.position.y)));
                    }
                    cx.notify();
                }),
            )
            .on_drag(StageDrag, |_, _, _, cx| cx.new(|_| StageGhost))
            .on_drag_move::<StageDrag>(cx.listener(
                |this, event: &gpui::DragMoveEvent<StageDrag>, _, _| {
                    if let Some(view) = this.overlay::<AccountView>() {
                        view.stage.drag_to(
                            f32::from(event.event.position.x),
                            f32::from(event.event.position.y),
                        );
                    }
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    if let Some(view) = this.overlay::<AccountView>() {
                        view.stage.grab = None;
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    if let Some(view) = this.overlay::<AccountView>() {
                        view.stage.grab = None;
                    }
                }),
            )
            .on_scroll_wheel(
                cx.listener(|this, event: &gpui::ScrollWheelEvent, window, _| {
                    let delta = f32::from(event.delta.pixel_delta(window.line_height()).y);
                    if let Some(view) = this.overlay::<AccountView>() {
                        view.stage.camera.wheel(-delta);
                    }
                }),
            );

        let download = |id: &'static str,
                        glyph: Icon,
                        size: f32,
                        tip: &'static str,
                        kind: TextureKind,
                        cx: &mut Context<Shell>| {
            let uuid = uuid.clone();
            let name = account.name.clone();
            Button::new(id, "", ButtonStyle::Tinted)
                .height(38.0)
                .grow()
                .icon(glyph, size)
                .tooltip(tip)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.download_texture(uuid.clone(), name.clone(), kind, cx)
                }))
        };
        let skin_col = div()
            .flex_none()
            .w(px(210.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(stage)
            .when(has_skin || has_cape, |col| {
                col.child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .when(has_skin, |row| {
                            row.child(download(
                                "dl-skin",
                                Icon::Skin,
                                18.0,
                                "Скачать скин",
                                TextureKind::Skin,
                                cx,
                            ))
                        })
                        .when(has_cape, |row| {
                            row.child(download(
                                "dl-cape",
                                Icon::Cape,
                                19.0,
                                "Скачать плащ",
                                TextureKind::Cape,
                                cx,
                            ))
                        }),
                )
            });

        let pin_key = Key::of("modal-pin");
        let pin_hover = motion::value(pin_key, cx);
        let pinned = account.pinned;
        let pin = with_tooltip(
            div()
                .id("modal-pin")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(28.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(p.line)
                .bg(mix(alpha(p.surface_2, 0.0), p.surface_2, pin_hover))
                .cursor_pointer()
                .on_hover(move |hovered, window, cx| motion::hover(pin_key, *hovered, window, cx))
                .on_click(cx.listener({
                    let uuid = uuid.clone();
                    move |this, _, _, cx| {
                        sound::play(Sound::Tap);
                        this.store
                            .update(cx, |store, cx| store.toggle_pin(&uuid, cx));
                    }
                }))
                .child(if pinned {
                    icon(Icon::StarSolid, px(17.0), p.yellow)
                } else {
                    icon(Icon::Star, px(17.0), mix(p.faint, p.muted, pin_hover))
                }),
            pin_key,
            if pinned {
                "Открепить"
            } else {
                "Закрепить"
            }
            .into(),
        );

        let name = if account.name.is_empty() {
            format!("{}…", account.short_id())
        } else {
            account.name.clone()
        };
        let name_row = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .pr(px(40.0))
            .child(pin)
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_size(rem(1.25))
                    .font_weight(FontWeight::BOLD)
                    .child(name),
            );

        let groups = (!ranks.is_empty()).then(|| {
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.0))
                .mt(px(12.0))
                .children(ranks.into_iter().map(|rank| {
                    let tone = parse_hex(&rank.color).unwrap_or(p.muted);
                    div()
                        .flex()
                        .items_center()
                        .px(px(9.0))
                        .py(px(4.0))
                        .rounded(px(7.0))
                        .border_1()
                        .border_color(alpha(tone, 0.3))
                        .bg(alpha(tone, 0.15))
                        .text_color(tone)
                        .text_size(rem(0.66))
                        .font_weight(FontWeight::BOLD)
                        .child(rank.label)
                }))
        });

        let label = div()
            .mt(px(12.0))
            .flex()
            .items_center()
            .px(px(11.0))
            .py(px(8.0))
            .rounded(px(9.0))
            .border_1()
            .border_color(if label_focused { p.blue } else { p.line })
            .bg(p.surface_2)
            .text_size(rem(0.8))
            .child(self.label.clone());

        let uuid_key = Key::of("uuid-row");
        let uuid_hover = motion::value(uuid_key, cx);
        let uuid_row = with_tooltip(
            div()
                .id("uuid-row")
                .flex()
                .items_center()
                .gap(px(8.0))
                .mt(px(8.0))
                .px(px(10.0))
                .py(px(8.0))
                .rounded(px(9.0))
                .border_1()
                .border_color(mix(p.line, p.line_strong, uuid_hover))
                .bg(mix(
                    p.surface_2,
                    if p.is_light() {
                        p.surface_2
                    } else {
                        rgb(0x262626)
                    },
                    uuid_hover,
                ))
                .cursor_pointer()
                .on_hover(move |hovered, window, cx| motion::hover(uuid_key, *hovered, window, cx))
                .on_click(cx.listener({
                    let uuid = uuid.clone();
                    move |this, _, _, cx| {
                        sound::play(Sound::Tap);
                        cx.write_to_clipboard(ClipboardItem::new_string(uuid.clone()));
                        if let Some(view) = this.overlay::<AccountView>() {
                            view.copied_at = Some(Instant::now());
                        }
                        this.toast("UUID скопирован", false, cx);
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(COPIED_FOR).await;
                            this.update(cx, |_, cx| cx.notify()).ok();
                        })
                        .detach();
                    }
                }))
                .child(
                    div()
                        .flex_none()
                        .text_size(rem(0.58))
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.faint)
                        .child(spaced("UUID", 0.4)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(rem(0.72))
                        .line_height(relative(1.4))
                        .child(account.uuid.clone()),
                )
                .child(icon(
                    Icon::Copy,
                    px(15.0),
                    if copied {
                        p.green
                    } else {
                        mix(p.faint, p.muted, uuid_hover)
                    },
                )),
            uuid_key,
            "Скопировать UUID".into(),
        );

        let rows = tiles.chunks(2).map(|pair| {
            div()
                .flex()
                .gap(px(8.0))
                .children(pair.iter().map(|tile| {
                    stat_tile(
                        div()
                            .text_size(rem(0.98))
                            .font(crate::theme::tabular(FontWeight::BOLD))
                            .when_some(tile.tone, |value, tone| value.text_color(tone))
                            .child(tile.value.clone()),
                        tile.label,
                        cx,
                    )
                    .flex_1()
                }))
                .when(pair.len() == 1, |row| row.child(div().flex_1()))
        });
        let stats = div().mt(px(16.0)).max_h(px(210.0)).flex().flex_col().child(
            ScrollArea::new("modal-stats", &self.stats_scroll)
                .fixed()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .pr(px(12.0))
                        .py(px(2.0))
                        .children(rows),
                ),
        );

        let mut options = vec![(String::new(), "- не выбран -".to_owned())];
        options.extend(profiles.iter().map(|name| (name.clone(), name.clone())));
        let current = if profiles.contains(&account.profile) {
            account.profile.clone()
        } else {
            String::new()
        };
        let profile_uuid = uuid.clone();
        let profile_row = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .mt(px(14.0))
            .child(field_label("Профиль", cx))
            .child(div().flex_1().min_w_0().child(shell.render_dropdown(
                "modal-profile",
                options,
                current,
                false,
                window,
                cx,
                move |this, value, _, cx| {
                    let uuid = profile_uuid.clone();
                    this.store.update(cx, |store, cx| {
                        let message = if value.is_empty() {
                            "Профиль убран".to_owned()
                        } else {
                            format!("Профиль назначен: {value}")
                        };
                        let future = store.backend.set_account_profile(uuid, value);
                        store.act(cx, future, message);
                    });
                },
            )));

        let auto_key = Key::of("ram-auto");
        let auto_hover = motion::value(auto_key, cx);
        let auto = with_tooltip(
            div()
                .id("ram-auto")
                .flex_none()
                .px(px(11.0))
                .py(px(5.0))
                .rounded(px(9.0))
                .border_1()
                .border_color(if auto_ram {
                    p.blue
                } else {
                    mix(p.line, p.line_strong, auto_hover)
                })
                .bg(if auto_ram { p.blue } else { p.surface_2 })
                .text_color(if auto_ram {
                    p.white
                } else {
                    mix(p.muted, p.text, auto_hover)
                })
                .text_size(rem(0.7))
                .font_weight(FontWeight::BOLD)
                .cursor_pointer()
                .on_hover(move |hovered, window, cx| motion::hover(auto_key, *hovered, window, cx))
                .on_click(cx.listener(|this, _, _, cx| {
                    sound::play(Sound::Tap);
                    this.toggle_auto_ram(cx);
                }))
                .child(spaced("Авто", 0.3)),
            auto_key,
            "Пусть лаунчер сам подберёт объём памяти".into(),
        );
        let ram_row = div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .mt(px(12.0))
            .child(field_label("RAM", cx))
            .child(auto)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .when(auto_ram, |s| s.opacity(0.35))
                    .child(
                        Slider::new("modal-ram-slider", ram, RAM_MIN, RAM_MAX, RAM_STEP)
                            .disabled(auto_ram)
                            .on_input(cx.listener(|this, value: &u32, _, cx| {
                                let value = *value;
                                if let Some(view) = this.overlay::<AccountView>() {
                                    view.ram_draft = Some(value);
                                    let input = view.ram_input.clone();
                                    input.update(cx, |input, cx| {
                                        input.set_text(value.to_string(), cx)
                                    });
                                }
                                cx.notify();
                            }))
                            .on_commit(cx.listener(|this, value: &u32, _, cx| {
                                this.save_modal_ram(*value, cx)
                            })),
                    ),
            )
            .child(ram_number(
                &self.ram_input,
                ram_focused,
                auto_ram,
                "modal",
                cx,
            ));

        let token = account.token_state();
        let note = match token {
            TokenState::Missing => {
                Some("Токен ещё не сохранён - зайди на этот аккаунт через лаунчер хотя бы раз.")
            }
            TokenState::Expired => Some(
                "Токен просрочен, вход может не сработать. Лаунчер попросит авторизацию заново.",
            ),
            TokenState::Ready => None,
        };
        let note = div()
            .mt(px(10.0))
            .min_h(rem(0.72))
            .text_size(rem(0.72))
            .text_color(if note.is_some() { p.yellow } else { p.faint })
            .children(note);

        let (launch_label, launch_style, launch_disabled) = if account.running {
            ("Закрыть", ButtonStyle::Stop, false)
        } else if account.launching {
            ("Запускается…", ButtonStyle::Primary, true)
        } else {
            ("Запуск", ButtonStyle::Primary, !account.launchable)
        };
        let actions = div()
            .flex()
            .gap(px(8.0))
            .mt_auto()
            .pt(px(18.0))
            .child(
                Button::new("modal-launch", launch_label, launch_style)
                    .grow()
                    .disabled(launch_disabled)
                    .on_click(cx.listener({
                        let uuid = uuid.clone();
                        move |this, _, window, cx| {
                            this.store
                                .update(cx, |store, cx| store.toggle_play(&uuid, cx));
                            this.close::<AccountView>(window, cx);
                        }
                    })),
            )
            .child(
                IconButton::new("modal-profile", Key::of("modal-profile"), IconStyle::Square)
                    .icon(Icon::Globe, 18.0)
                    .size(42.0, 41.0)
                    .tooltip("Открыть профиль на сайте")
                    .on_click({
                        let name = account.name.clone();
                        move |_, _, _| {
                            if !name.is_empty() {
                                platform::open_url(&format!(
                                    "https://top.cristalix.gg/profile/{name}"
                                ));
                            }
                        }
                    }),
            )
            .child(
                div().flex().items_center().child(
                    IconButton::new("modal-forget", Key::of("modal-forget"), IconStyle::Danger)
                        .icon(Icon::Trash, 17.0)
                        .size(40.0, 41.0)
                        .tooltip("Удалить аккаунт")
                        .on_click(cx.listener({
                            let uuid = uuid.clone();
                            move |this, _, window, cx| {
                                this.confirm_forget(uuid.clone(), true, window, cx)
                            }
                        })),
                ),
            );

        let info = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(name_row)
            .children(groups)
            .child(label)
            .child(uuid_row)
            .child(stats)
            .child(profile_row)
            .child(ram_row)
            .child(note)
            .child(actions);

        let compact = f32::from(window.viewport_size().width) <= crate::theme::COMPACT_WIDTH;
        ModalFrame::new("account-overlay", 600.0, Shell::closer::<AccountView>(cx))
            .unfold()
            .max_width_fraction(1.0)
            .child(
                div()
                    .flex()
                    .gap(px(18.0))
                    .px(px(20.0))
                    .py(px(24.0))
                    .when(compact, |top| top.flex_col().items_center())
                    .child(skin_col)
                    .child(info),
            )
            .into_any_element()
    }
}

pub fn ram_number(
    input: &Entity<TextInput>,
    focused: bool,
    disabled: bool,
    scope: &'static str,
    cx: &mut Context<Shell>,
) -> impl IntoElement + use<> {
    let p = cx.palette();
    let step = |direction: i32, cx: &mut Context<Shell>| {
        let key = Key::new("ram-step", (scope, direction));
        let hover = motion::value(key, cx);
        div()
            .id(ElementId::Name(format!("{scope}-ram-{direction}").into()))
            .flex()
            .items_center()
            .justify_center()
            .w(px(20.0))
            .h(px(15.0))
            .rounded(px(5.0))
            .border_1()
            .border_color(mix(p.line, p.blue, hover))
            .bg(mix(p.surface_2, rgba(0x3b82e8, 0.14), hover))
            .cursor_pointer()
            .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
            .on_click(cx.listener(move |this, _, _, cx| {
                sound::play(Sound::Tap);
                this.step_ram(scope, direction, cx);
            }))
            .child(icon(
                if direction > 0 {
                    Icon::CaretUp
                } else {
                    Icon::CaretDownBold
                },
                px(10.0),
                mix(p.muted, p.text, hover),
            ))
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(5.0))
        .when(disabled, |row| row.opacity(0.35))
        .child(
            div()
                .w(px(74.0))
                .flex()
                .px(px(8.0))
                .py(px(6.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(if focused { p.blue } else { p.line })
                .bg(p.surface_2)
                .text_size(rem(0.82))
                .font_weight(FontWeight::SEMIBOLD)
                .child(input.clone()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(step(1, cx))
                .child(step(-1, cx)),
        )
        .child(
            div()
                .text_size(rem(0.72))
                .font_weight(FontWeight::BOLD)
                .text_color(p.faint)
                .child("МБ"),
        )
}

impl Shell {
    pub(crate) fn open_account(
        &mut self,
        uuid: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(account) = self.store.read(cx).account(&uuid).cloned() else {
            return;
        };
        let label = cx.new(|cx| {
            let mut input = TextInput::new("Метка (до 8 символов)", window, cx)
                .max_chars(ac_api::LABEL_MAX_CHARS);
            input.set_text(account.label.clone(), cx);
            input
        });
        cx.subscribe_in(
            &label,
            window,
            |this, input, event, window, cx| match event {
                InputEvent::Submit => window.focus(&this.focus_handle(cx)),
                InputEvent::Blur => {
                    let value = input.read(cx).text().trim().to_owned();
                    if let Some(view) = this.overlay::<AccountView>() {
                        let uuid = view.uuid.clone();
                        let current = this.store.read(cx).account(&uuid).map(|a| a.label.clone());
                        if current.is_some_and(|current| current != value) {
                            this.store.update(cx, |store, cx| {
                                let future = store.backend.set_label(uuid, value);
                                store.act(cx, future, "");
                            });
                        }
                    }
                }
                _ => {}
            },
        )
        .detach();
        let ram_input = cx.new(|cx| {
            let mut input = TextInput::new("", window, cx)
                .digits()
                .max_chars(5)
                .align_right();
            input.set_text(ram_of(&account).to_string(), cx);
            input
        });
        cx.subscribe_in(&ram_input, window, |this, input, event, window, cx| {
            if matches!(event, InputEvent::Submit | InputEvent::Blur) {
                let value = input.read(cx).text().parse::<u32>().unwrap_or(RAM_MIN);
                if *event == InputEvent::Submit {
                    window.focus(&this.focus_handle(cx));
                } else {
                    this.save_modal_ram(value, cx);
                }
            }
        })
        .detach();
        self.store.update(cx, |store, cx| {
            store.ensure_player(&account.name, cx);
            store.reload_profiles(cx);
        });
        let cape = self
            .store
            .read(cx)
            .backend
            .texture(uuid.clone(), TextureKind::Cape);
        let owner = uuid.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(Some(bytes)) = cape.await {
                this.update(cx, |shell, cx| {
                    if let Some(view) = shell
                        .overlay::<AccountView>()
                        .filter(|view| view.uuid == owner)
                    {
                        view.stage.cape = ac_skin::Cape::from_png(&bytes).ok().map(Arc::new);
                        view.cape = Some(bytes);
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
        let stage_task = self.spawn_stage(uuid.clone(), window, cx);
        self.push_overlay(
            super::shell::Overlay::Account(AccountView {
                uuid,
                label,
                ram_input,
                ram_draft: None,
                cape: None,
                stage: Stage::default(),
                _stage_task: stage_task,
                copied_at: None,
                stats_scroll: ScrollHandle::new(),
            }),
            cx,
        );
    }

    fn spawn_stage(
        &mut self,
        uuid: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            let mut shown_at: Option<Instant> = None;
            loop {
                let step = this.update_in(cx, |shell, window, cx| {
                    let live = window.is_window_active();
                    let skin = match shell.store.read(cx).skin_texture(&uuid) {
                        Some(Texture::Ready(skin)) => Some(skin),
                        _ => None,
                    };
                    let scale = window.scale_factor();
                    let view = shell.overlay::<AccountView>()?;
                    live.then_some(())?;
                    let request = skin.and_then(|skin| view.stage.request(skin, scale));
                    Some((request, next_frame(window)))
                });
                let Ok(step) = step else {
                    break;
                };
                let Some((request, mut frame_shown)) = step else {
                    cx.background_executor().timer(STAGE_IDLE).await;
                    continue;
                };
                let Some(request) = request else {
                    frame_shown.await.ok();
                    continue;
                };
                let frame = cx
                    .background_executor()
                    .spawn(async move { request.render() })
                    .await;
                loop {
                    frame_shown.await.ok();
                    if shown_at.is_none_or(|at| at.elapsed() >= STAGE_FRAME) {
                        break;
                    }
                    let Ok(next) = this.update_in(cx, |_, window, _| next_frame(window)) else {
                        return;
                    };
                    frame_shown = next;
                }
                let applied = this.update_in(cx, |shell, window, cx| {
                    let Some(view) = shell.overlay::<AccountView>() else {
                        return;
                    };
                    let previous = std::mem::replace(&mut view.stage.frame, frame);
                    if let Some(previous) = previous {
                        cx.drop_image(previous, Some(window));
                    }
                    cx.notify();
                });
                if applied.is_err() {
                    break;
                }
                shown_at = Some(Instant::now());
            }
        })
    }

    fn download_texture(
        &mut self,
        uuid: String,
        name: String,
        kind: TextureKind,
        cx: &mut Context<Self>,
    ) {
        let future = self.store.read(cx).backend.texture(uuid.clone(), kind);
        let file = format!(
            "{}_{}.png",
            if name.is_empty() { &uuid } else { &name },
            kind.as_str()
        );
        cx.spawn(async move |this, cx| {
            let Ok(Some(bytes)) = future.await else {
                return;
            };
            let dialog = rfd::AsyncFileDialog::new()
                .set_file_name(&file)
                .add_filter("PNG", &["png"])
                .save_file()
                .await;
            if let Some(handle) = dialog {
                let saved = std::fs::write(handle.path(), bytes).is_ok();
                this.update(cx, |shell, cx| {
                    if saved {
                        shell.toast(format!("Сохранено: {file}"), false, cx);
                    } else {
                        shell.toast("Не удалось сохранить файл", true, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn toggle_auto_ram(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<AccountView>() else {
            return;
        };
        let uuid = view.uuid.clone();
        let typed = view
            .ram_input
            .read(cx)
            .text()
            .parse::<u32>()
            .unwrap_or(RAM_DEFAULT);
        let Some(account) = self.store.read(cx).account(&uuid).cloned() else {
            return;
        };
        let ram = if account.launch.auto_ram() {
            snap_ram(typed)
        } else {
            0
        };
        if let Some(view) = self.overlay::<AccountView>() {
            view.ram_draft = None;
            if ram > 0 {
                view.ram_input
                    .update(cx, |input, cx| input.set_text(ram.to_string(), cx));
            }
        }
        self.store.update(cx, |store, cx| {
            let future = store.backend.set_ram(vec![uuid], ram);
            store.act(cx, future, "");
        });
    }

    fn save_modal_ram(&mut self, value: u32, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<AccountView>() else {
            return;
        };
        let ram = snap_ram(value);
        let uuid = view.uuid.clone();
        view.ram_draft = None;
        view.ram_input
            .update(cx, |input, cx| input.set_text(ram.to_string(), cx));
        let current = self.store.read(cx).account(&uuid).map(|a| a.launch.ram);
        if current == Some(ram) {
            return;
        }
        self.store.update(cx, |store, cx| {
            if let Some(account) = store.accounts.iter_mut().find(|a| a.uuid == uuid) {
                account.launch.ram = ram;
            }
            let future = store.backend.set_ram(vec![uuid], ram);
            store.act(cx, future, "");
        });
        cx.notify();
    }

    pub(crate) fn step_ram(&mut self, scope: &'static str, direction: i32, cx: &mut Context<Self>) {
        if scope == "modal" {
            let Some(view) = self.overlay::<AccountView>() else {
                return;
            };
            let current = view
                .ram_input
                .read(cx)
                .text()
                .parse::<u32>()
                .unwrap_or(RAM_DEFAULT);
            let next = snap_ram(
                (snap_ram(current) as i64 + direction as i64 * RAM_STEP as i64).max(0) as u32,
            );
            self.save_modal_ram(next, cx);
        } else {
            self.step_group_ram(direction, cx);
        }
    }
}

fn next_frame(window: &Window) -> oneshot::Receiver<()> {
    let (vsync, frame_shown) = oneshot::channel();
    window.on_next_frame(move |_, _| {
        vsync.send(()).ok();
    });
    frame_shown
}
