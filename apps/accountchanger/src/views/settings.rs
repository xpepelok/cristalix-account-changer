use ac_api::{
    BACKGROUND_BLUR_MAX, BACKGROUND_DIM_MAX, BACKGROUND_EXTENSIONS, LauncherKind, Os, Settings,
};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::format::file_name;
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, mix, rem, rgba};
use crate::ui::{ModalFrame, Slider, Toggle, spaced};
use crate::views::shell::{Overlay, Shell};

#[derive(Default)]
pub struct SettingsView {
    dim: Option<u8>,
    blur: Option<u8>,
}

#[derive(Clone, Copy)]
enum Switch {
    AutoPlay,
    Aggressive,
    WindowTitle,
    Autostart,
    Sound,
    Stats,
}

fn launcher_label(kind: LauncherKind, menu: bool) -> &'static str {
    match kind {
        LauncherKind::Normal => "Обычный лаунчер",
        LauncherKind::Jar => "JAR-версия лаунчера",
        LauncherKind::New => "Новый лаунчер",
        LauncherKind::Custom if menu => "Свой лаунчер…",
        LauncherKind::Custom => "Свой лаунчер",
    }
}

pub fn small_button(
    id: &'static str,
    label: &'static str,
    danger: bool,
    cx: &mut Context<Shell>,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.palette();
    let key = Key::of(id);
    let hover = motion::value(key, cx);
    let (fg, border, bg) = if danger {
        (
            mix(p.muted, p.red, hover),
            mix(p.line, p.red, hover),
            mix(rgba(0x000000, 0.0), gpui::Hsla { a: 0.1, ..p.red }, hover),
        )
    } else {
        (
            mix(p.muted, p.text, hover),
            mix(p.line, p.line_strong, hover),
            mix(rgba(0x000000, 0.0), p.wash, hover),
        )
    };
    div()
        .id(id)
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

const WIDE_MIN_VIEWPORT: f32 = 880.0;
const WIDE_WIDTH: f32 = 820.0;
const NARROW_WIDTH: f32 = 420.0;

impl SettingsView {
    pub fn render(
        &mut self,
        shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let store = shell.store.read(cx);
        let settings = store.settings.clone();
        let caps = store.caps;
        let windows = caps.os == Os::Windows;

        let section = |title: &'static str| {
            div()
                .px(px(2.0))
                .pb(px(6.0))
                .text_size(rem(0.68))
                .font_weight(FontWeight::BOLD)
                .text_color(p.faint)
                .child(spaced(&title.to_uppercase(), 0.7))
        };
        let card = || {
            div()
                .flex()
                .flex_col()
                .rounded(px(12.0))
                .border_1()
                .border_color(p.line)
                .bg(p.surface_2)
        };
        let info = |label: &'static str, desc: SharedString| {
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(rem(0.85))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(rem(0.72))
                        .line_height(gpui::relative(1.35))
                        .text_color(p.muted)
                        .child(desc),
                )
        };
        let row = |first: bool| {
            div()
                .flex()
                .items_center()
                .gap(px(14.0))
                .px(px(14.0))
                .py(px(10.0))
                .when(!first, |r| r.border_t_1().border_color(p.line))
        };
        let switch = |id: &'static str, on: bool, which: Switch, cx: &mut Context<Shell>| {
            Toggle::new(id, Key::of(id), on).on_toggle(
                cx.listener(move |this, next: &bool, _, cx| this.set_switch(which, *next, cx)),
            )
        };

        let launcher_options: Vec<(String, String)> = LauncherKind::ALL
            .into_iter()
            .filter(|kind| caps.exe_launchers || !kind.is_exe())
            .map(|kind| (format!("{kind:?}"), launcher_label(kind, true).to_owned()))
            .collect();
        let current = format!("{:?}", settings.launcher);
        let launcher_dropdown = shell
            .render_dropdown(
                "launcher-dd",
                launcher_options,
                current,
                false,
                window,
                cx,
                |this, value, window, cx| {
                    let kind = LauncherKind::ALL
                        .into_iter()
                        .find(|kind| format!("{kind:?}") == value)
                        .unwrap_or_default();
                    this.choose_launcher(kind, window, cx);
                },
            )
            .into_any_element();
        let custom_path = settings.custom_launcher.clone();
        let launcher_row = div()
            .flex()
            .flex_col()
            .gap(px(9.0))
            .px(px(14.0))
            .py(px(10.0))
            .child(info(
                "Лаунчер",
                "Чем запускать аккаунты и «Открыть лаунчер»".into(),
            ))
            .child(launcher_dropdown)
            .when(settings.launcher == LauncherKind::Custom, |col| {
                col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .mt(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .text_size(px(12.0))
                                .text_color(gpui::rgb(0x97a1b0))
                                .child(if custom_path.is_empty() {
                                    "Файл не выбран".to_owned()
                                } else {
                                    file_name(&custom_path).to_owned()
                                }),
                        )
                        .child(
                            small_button("custom-launcher-browse", "Обзор", false, cx).on_click(
                                cx.listener(|this, _, _, cx| {
                                    sound::play(Sound::Tap);
                                    this.pick_custom_launcher(cx);
                                }),
                            ),
                        ),
                )
            });

        let mut launch = card().child(launcher_row);
        if caps.auto_play {
            launch = launch.child(
                row(false)
                    .child(info(
                        "Автозапуск клиента",
                        "Открывать клиент сразу после входа в аккаунт".into(),
                    ))
                    .child(switch(
                        "toggle-autoplay",
                        settings.auto_play,
                        Switch::AutoPlay,
                        cx,
                    )),
            );
        }
        launch = launch.child(
            row(false)
                .child(info(
                    "Агрессивный запуск",
                    "Массово, без очереди - сильно грузит ПК".into(),
                ))
                .child(switch(
                    "toggle-aggressive",
                    settings.aggressive,
                    Switch::Aggressive,
                    cx,
                )),
        );
        if windows {
            launch = launch.child(
                row(false)
                    .child(info(
                        "Ник в заголовке окна",
                        "Переименовывать окно клиента в ник аккаунта".into(),
                    ))
                    .child(switch(
                        "toggle-window-title",
                        settings.window_title,
                        Switch::WindowTitle,
                        cx,
                    )),
            );
        }

        let autostart_desc = if windows {
            "Запускать AccountChanger при включении Windows"
        } else {
            "Запускать AccountChanger при входе в систему"
        };
        let app = card()
            .child(
                row(true)
                    .child(info("Автозапуск", autostart_desc.into()))
                    .child(switch(
                        "toggle-autostart",
                        settings.autostart,
                        Switch::Autostart,
                        cx,
                    )),
            )
            .child(
                row(false)
                    .child(info(
                        "Звуки интерфейса",
                        "Мягкие звуки при нажатии на кнопки".into(),
                    ))
                    .child(switch("toggle-sound", settings.sound, Switch::Sound, cx)),
            )
            .child(
                row(false)
                    .child(info(
                        "Отправка метрик",
                        "Без неё не виден актуальный онлайн".into(),
                    ))
                    .child(switch("toggle-stats", settings.stats, Switch::Stats, cx)),
            );

        let wide = f32::from(window.viewport_size().width) >= WIDE_MIN_VIEWPORT;
        let background = self.render_background(&settings, wide, cx);
        let column = |title: &'static str, content: gpui::Div| {
            div()
                .flex()
                .flex_col()
                .when(wide, |column| column.flex_1().min_w_0())
                .child(section(title))
                .child(content.when(wide, |card| card.flex_1()))
        };
        let launch = column("Запуск", launch);
        let app = column("Приложение", app);
        let groups = if wide {
            div()
                .flex()
                .gap(px(14.0))
                .child(launch)
                .child(app)
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .child(launch)
                .child(app)
                .into_any_element()
        };

        let width = if wide { WIDE_WIDTH } else { NARROW_WIDTH };
        ModalFrame::new("settings-overlay", width, Shell::closer::<SettingsView>(cx))
            .padding(0.0, 0.0, 14.0, 0.0)
            .child(
                div()
                    .pt(px(16.0))
                    .px(px(18.0))
                    .pb(px(12.0))
                    .border_b_1()
                    .border_color(p.line)
                    .child(
                        div()
                            .pr(px(40.0))
                            .text_size(rem(1.05))
                            .font_weight(FontWeight::BOLD)
                            .child("Настройки"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .pt(px(6.0))
                    .px(px(18.0))
                    .pb(px(16.0))
                    .child(groups)
                    .child(div().child(section("Фон")).child(background)),
            )
            .into_any_element()
    }

    fn render_background(
        &mut self,
        settings: &Settings,
        wide: bool,
        cx: &mut Context<Shell>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let background = &settings.background;
        let dim = self.dim.unwrap_or(background.dim);
        let blur = self.blur.unwrap_or(background.blur);
        let has_file = background.file.is_some();
        let slot = || div().flex().flex_col().flex_1().min_w_0().gap(px(8.0));
        let label = |text: &'static str, value: String| {
            div()
                .flex()
                .justify_between()
                .text_size(rem(0.85))
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
                .child(div().text_color(p.muted).text_size(rem(0.78)).child(value))
        };
        div()
            .flex()
            .flex_col()
            .rounded(px(12.0))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .px(px(14.0))
                    .py(px(10.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(rem(0.85))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Свой фон"),
                            )
                            .child(
                                div()
                                    .mt(px(2.0))
                                    .text_size(rem(0.72))
                                    .text_color(p.muted)
                                    .overflow_hidden()
                                    .child(if has_file {
                                        "Установлен".to_owned()
                                    } else {
                                        "PNG, JPG, WEBP или GIF".to_owned()
                                    }),
                            ),
                    )
                    .child(
                        small_button("background-pick", "Выбрать", false, cx).on_click(
                            cx.listener(|this, _, _, cx| {
                                sound::play(Sound::Tap);
                                this.pick_background(cx);
                            }),
                        ),
                    )
                    .when(has_file, |row| {
                        row.child(
                            small_button("background-reset", "Сбросить", true, cx).on_click(
                                cx.listener(|this, _, _, cx| {
                                    sound::play(Sound::Tap);
                                    this.reset_background(cx);
                                }),
                            ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .when(!wide, |sliders| sliders.flex_col())
                    .gap(px(if wide { 24.0 } else { 14.0 }))
                    .px(px(14.0))
                    .py(px(10.0))
                    .border_t_1()
                    .border_color(p.line)
                    .when(!has_file, |sliders| sliders.hidden())
                    .child(
                        slot().child(label("Затемнение", format!("{dim}%"))).child(
                            Slider::new(
                                "background-dim",
                                dim as u32,
                                0,
                                BACKGROUND_DIM_MAX as u32,
                                1,
                            )
                            .track(p.line_strong)
                            .on_input(cx.listener(|this, value: &u32, _, cx| {
                                let dim = *value as u8;
                                if let Some(view) = this.overlay::<SettingsView>() {
                                    view.dim = Some(dim);
                                }
                                this.store.update(cx, |store, cx| {
                                    store.settings.background.dim = dim;
                                    cx.notify();
                                });
                            }))
                            .on_commit(cx.listener(
                                |this, value: &u32, _, cx| this.commit_dim(*value as u8, cx),
                            )),
                        ),
                    )
                    .child(
                        slot().child(label("Размытие", format!("{blur} px"))).child(
                            Slider::new(
                                "background-blur",
                                blur as u32,
                                0,
                                BACKGROUND_BLUR_MAX as u32,
                                1,
                            )
                            .track(p.line_strong)
                            .on_input(cx.listener(|this, value: &u32, _, cx| {
                                if let Some(view) = this.overlay::<SettingsView>() {
                                    view.blur = Some(*value as u8);
                                }
                                cx.notify();
                            }))
                            .on_commit(cx.listener(
                                |this, value: &u32, _, cx| this.commit_blur(*value as u8, cx),
                            )),
                        ),
                    ),
            )
    }
}

impl Shell {
    pub(crate) fn open_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            let future = store.backend.settings();
            store.fetch(cx, future, |store, settings, cx| {
                store.settings = settings;
                cx.notify();
            });
        });
        self.push_overlay(Overlay::Settings(SettingsView::default()), cx);
    }

    fn set_switch(&mut self, which: Switch, next: bool, cx: &mut Context<Self>) {
        let (message_on, message_off) = match which {
            Switch::AutoPlay => (
                "Автонажатие ИГРАТЬ включено",
                "Автонажатие ИГРАТЬ выключено",
            ),
            Switch::Aggressive => ("Агрессивный запуск включён", "Агрессивный запуск выключен"),
            Switch::WindowTitle => (
                "Окно клиента будет называться ником",
                "Заголовок окна клиента вернётся к обычному",
            ),
            Switch::Autostart => ("Автозапуск включён", "Автозапуск выключен"),
            Switch::Sound => ("Звуки включены", "Звуки выключены"),
            Switch::Stats => ("Отправка метрик включена", "Отправка метрик выключена"),
        };
        self.store.update(cx, |store, cx| {
            let settings = &mut store.settings;
            match which {
                Switch::AutoPlay => settings.auto_play = next,
                Switch::Aggressive => settings.aggressive = next,
                Switch::WindowTitle => settings.window_title = next,
                Switch::Autostart => settings.autostart = next,
                Switch::Sound => {
                    settings.sound = next;
                    sound::set_enabled(next);
                    if next {
                        sound::play(Sound::Tap);
                    }
                }
                Switch::Stats => settings.stats = next,
            }
            cx.notify();
            let backend = store.backend.clone();
            let applied = move |store: &mut crate::state::Store,
                                value: bool,
                                cx: &mut Context<crate::state::Store>| {
                match which {
                    Switch::AutoPlay => store.settings.auto_play = value,
                    Switch::Aggressive => store.settings.aggressive = value,
                    Switch::WindowTitle => store.settings.window_title = value,
                    Switch::Autostart => store.settings.autostart = value,
                    Switch::Stats => {
                        store.settings.stats = value;
                        let future = store.backend.online_stats();
                        store.fetch(cx, future, |store, stats, cx| {
                            store.online = Some(stats);
                            cx.notify();
                        });
                    }
                    Switch::Sound => {}
                }
                store.toast(if next { message_on } else { message_off }, false, cx);
            };
            match which {
                Switch::Sound => {
                    let future = backend.set_sound(next);
                    store.act(cx, future, if next { message_on } else { message_off });
                }
                Switch::AutoPlay => store.fetch(cx, backend.set_auto_play(next), applied),
                Switch::Aggressive => store.fetch(cx, backend.set_aggressive(next), applied),
                Switch::WindowTitle => store.fetch(cx, backend.set_window_title(next), applied),
                Switch::Autostart => store.fetch(cx, backend.set_autostart(next), applied),
                Switch::Stats => store.fetch(cx, backend.set_stats(next), applied),
            }
        });
    }

    fn choose_launcher(
        &mut self,
        kind: LauncherKind,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let settings = self.store.read(cx).settings.clone();
        if kind == LauncherKind::Custom && settings.custom_launcher.is_empty() {
            self.pick_custom_launcher(cx);
            return;
        }
        if kind == settings.launcher {
            return;
        }
        self.store.update(cx, |store, cx| {
            let previous = store.settings.launcher;
            store.settings.launcher = kind;
            cx.notify();
            let future = store.backend.set_launcher(kind);
            cx.spawn(async move |store, cx| {
                let result = future.await;
                store
                    .update(cx, |store, cx| match result {
                        Ok(kind) => {
                            store.settings.launcher = kind;
                            let label = if kind == LauncherKind::Custom {
                                "свой"
                            } else {
                                launcher_label(kind, false)
                            };
                            store.toast(format!("Лаунчер: {label}"), false, cx);
                        }
                        Err(error) => {
                            store.settings.launcher = previous;
                            store.toast(error.message().to_owned(), true, cx);
                        }
                    })
                    .ok();
            })
            .detach();
        });
    }

    fn pick_custom_launcher(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("Свой лаунчер");
            if cfg!(windows) {
                dialog = dialog.add_filter("Лаунчер", &["exe", "jar", "bat", "cmd", "lnk"]);
            }
            let Some(file) = dialog.pick_file().await else {
                return;
            };
            let path = file.path().to_path_buf();
            store
                .update(cx, |store, cx| {
                    let future = store.backend.set_custom_launcher(path);
                    store.fetch(cx, future, |store, result, cx| {
                        let name = file_name(&result.custom_launcher).to_owned();
                        store.settings.launcher = result.launcher;
                        store.settings.custom_launcher = result.custom_launcher;
                        store.toast(format!("Свой лаунчер: {name}"), false, cx);
                    });
                })
                .ok();
        })
        .detach();
    }

    fn pick_background(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("Фон приложения")
                .add_filter("Изображения", &BACKGROUND_EXTENSIONS)
                .pick_file()
                .await
            else {
                return;
            };
            let path = file.path().to_path_buf();
            store
                .update(cx, |store, cx| {
                    let future = store.backend.set_background(Some(path));
                    store.fetch(cx, future, |store, name, cx| {
                        store.settings.background.file = name;
                        store.toast("Фон обновлён", false, cx);
                    });
                })
                .ok();
        })
        .detach();
    }

    fn reset_background(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.overlay::<SettingsView>() {
            view.dim = None;
            view.blur = None;
        }
        self.store.update(cx, |store, cx| {
            let future = store.backend.set_background(None);
            store.fetch(cx, future, |store, _, cx| {
                store.settings.background = ac_api::Background::default();
                store.toast("Фон сброшен", false, cx);
            });
        });
    }

    fn commit_dim(&mut self, dim: u8, cx: &mut Context<Self>) {
        if let Some(view) = self.overlay::<SettingsView>() {
            view.dim = None;
        }
        self.store.update(cx, |store, cx| {
            store.settings.background.dim = dim;
            cx.notify();
            let future = store.backend.set_background_dim(dim);
            store.act(cx, future, "");
        });
    }

    fn commit_blur(&mut self, blur: u8, cx: &mut Context<Self>) {
        if let Some(view) = self.overlay::<SettingsView>() {
            view.blur = None;
        }
        self.store.update(cx, |store, cx| {
            store.settings.background.blur = blur;
            cx.notify();
            let future = store.backend.set_background_blur(blur);
            store.act(cx, future, "");
        });
    }
}
