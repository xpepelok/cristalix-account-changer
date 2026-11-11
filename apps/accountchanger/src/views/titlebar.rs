use std::time::Duration;

use ac_api::{OnlineStats, Theme};
use gpui::{
    AnimationExt, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Radians, StatefulInteractiveElement, Styled, Transformation, Window, WindowControlArea, div,
    point, prelude::FluentBuilder, px, size,
};

use crate::ambient;
use crate::assets::{Icon, icon};
use crate::motion::{self, Key};
use crate::platform;
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, alpha, mix, rem, rgb, rgba};
use crate::ui::{IconButton, IconStyle, spaced, with_tooltip};
use crate::views::shell::{Popup, Shell};

pub const TITLEBAR_HEIGHT: f32 = 54.0;
const STATS_PULSE: Key = Key::of("stats-bars");
const STATS_BAR: f32 = 0.25;
const STATS_ENTER: f32 = 0.37;
const PROFILES_PULSE: Key = Key::of("profiles-lift");
const THEME_POP: Key = Key::of("theme-pop");
const THEME_SWAP: Key = Key::of("theme-swap");
const BELL_SWING: Key = Key::of("bell-swing");
const POP_CURVE: [f32; 4] = [0.34, 1.56, 0.64, 1.0];
const POP_ROTATE: [(f32, f32); 4] = [(0.0, 0.0), (0.35, 28.0), (0.65, -14.0), (1.0, 0.0)];
const POP_SCALES: [(f32, f32); 4] = [(0.0, 1.0), (0.35, 1.2), (0.65, 1.06), (1.0, 1.0)];
const SWING_CURVE: [f32; 4] = [0.36, 0.07, 0.19, 0.97];
const SWING: [(f32, f32); 7] = [
    (0.0, 0.0),
    (0.16, 9.0),
    (0.32, -8.0),
    (0.48, 6.0),
    (0.64, -4.0),
    (0.8, 2.0),
    (1.0, 0.0),
];
const RING: [(f32, f32); 9] = [
    (0.0, 0.0),
    (0.02, 13.0),
    (0.05, -11.0),
    (0.08, 9.0),
    (0.11, -6.0),
    (0.14, 4.0),
    (0.17, -2.0),
    (0.19, 0.0),
    (1.0, 0.0),
];
const ICON_SCALE: f32 = 17.0 / 24.0;
const BARS: [(f32, f32, f32, f32); 3] = [
    (5.5, 7.0, 1.3, 0.0),
    (12.0, 11.0, 1.2, 0.06),
    (18.5, 15.0, 1.12, 0.12),
];
const BAR_WIDTH: f32 = 2.4;
const BAR_BOTTOM: f32 = 20.0;

fn stats_bars(progress: f32, entering: bool, color: gpui::Hsla, scale: f32) -> gpui::Div {
    let snap = |value: f32| (value * scale).round() / scale;
    let stroke = snap(BAR_WIDTH * ICON_SCALE).max(1.0 / scale);
    let bottom = snap(BAR_BOTTOM * ICON_SCALE);
    div()
        .relative()
        .size(px(17.0))
        .children(BARS.iter().map(|(x, length, grow, delay)| {
            let factor = if entering {
                let local = ((progress * STATS_ENTER - delay) / STATS_BAR).clamp(0.0, 1.0);
                motion::cubic_bezier(motion::EASE, local)
            } else {
                1.0 - motion::cubic_bezier(motion::EASE, 1.0 - progress)
            };
            let height = length * (1.0 + (grow - 1.0) * factor) * ICON_SCALE;
            div()
                .absolute()
                .left(px(snap(x * ICON_SCALE - stroke / 2.0)))
                .top(px(bottom - height))
                .w(px(stroke))
                .h(px(height))
                .rounded_t(px(stroke / 2.0))
                .rounded_b(px(stroke / 4.0))
                .bg(color)
        }))
}
const CRISTALIX_URL: &str = "https://cristalix.gg";
const REPOSITORY_URL: &str = "https://github.com/xpepelok/cristalix-account-changer";

impl Shell {
    pub(crate) fn render_titlebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let online = store.online;
        let theme = store.settings.theme;
        let has_update = store.update.as_ref().is_some_and(|u| u.available);
        let tray = store.caps.tray;
        let maximized = window.is_maximized();
        let gutter = self.gutter(window);
        let narrow = f32::from(window.viewport_size().width) <= crate::theme::NARROW_WIDTH;
        let logo_key = Key::of("logo");
        let logo_hover = motion::value(logo_key, cx);
        let brand_key = Key::of("brand");
        let brand_hover = motion::value(brand_key, cx);

        let logo = div()
            .id("logo")
            .flex()
            .items_center()
            .cursor_pointer()
            .occlude()
            .opacity(1.0 - 0.2 * logo_hover)
            .on_hover(move |hovered, window, cx| motion::hover(logo_key, *hovered, window, cx))
            .on_click(|_, _, _| {
                sound::play(Sound::Tap);
                platform::open_url(CRISTALIX_URL);
            })
            .child(
                icon(
                    Icon::Logo,
                    px(22.0),
                    if p.is_light() {
                        rgb(0x000000)
                    } else {
                        rgb(0xffffff)
                    },
                )
                .w(px(136.59)),
            );

        let brand = div()
            .id("brand")
            .flex()
            .items_center()
            .px(px(9.0))
            .py(px(3.0))
            .rounded_full()
            .bg(rgba(0x3b82e8, 0.14 + 0.10 * brand_hover))
            .text_color(p.blue)
            .text_size(rem(0.72))
            .font_weight(FontWeight::BOLD)
            .cursor_pointer()
            .occlude()
            .on_hover(move |hovered, window, cx| motion::hover(brand_key, *hovered, window, cx))
            .on_click(|_, _, _| {
                sound::play(Sound::Tap);
                platform::open_url(REPOSITORY_URL);
            })
            .child(spaced("ACCOUNTCHANGER", 0.6));
        let brand = with_tooltip(brand, brand_key, "Репозиторий на GitHub".into());

        let badge = online.map(|stats| self.render_online_badge(stats, cx));

        let round = |id: &'static str, glyph: Icon, size: f32, tip: &'static str| {
            IconButton::new(id, Key::of(id), IconStyle::Round)
                .icon(glyph, size)
                .tooltip(tip)
        };

        let theme_tip = if theme == Theme::Dark {
            "Светлая тема"
        } else {
            "Тёмная тема"
        };
        let bell_shown = self.popup_shown(&Popup::Bell);
        let bell_open = bell_shown.is_some();
        let bell_active = bell_shown == Some(None);
        let light = theme == Theme::Light;
        if motion::is_on(THEME_SWAP, cx) != light && motion::running(THEME_SWAP, cx).is_none() {
            motion::drive(THEME_SWAP, light, Duration::ZERO, motion::EASE, window, cx);
        }
        let hover_color = |id: &'static str, cx: &Context<Self>| {
            let t = motion::value(Key::of(id), cx);
            mix(p.muted, p.text, t)
        };

        let bars_progress = motion::value(STATS_PULSE, cx);
        let stats_color = hover_color("stats-btn", cx);
        let stats_icon = stats_bars(
            bars_progress,
            motion::is_on(STATS_PULSE, cx),
            stats_color,
            window.scale_factor(),
        );

        let logs_color = hover_color("logs-btn", cx);
        let logs_hovered = motion::is_on(Key::of("logs-btn"), cx);
        let cursor_on =
            !logs_hovered || ambient::cycle(Duration::from_millis(900), cx).is_none_or(|t| t < 0.5);
        let logs_icon = div()
            .relative()
            .size(px(17.0))
            .child(icon(Icon::LogsFrame, px(17.0), logs_color).absolute())
            .when(cursor_on, |frame| {
                frame.child(
                    div()
                        .absolute()
                        .left(px(8.3))
                        .top(px(10.06))
                        .w(px(4.53))
                        .h(px(1.13))
                        .rounded_full()
                        .bg(logs_color),
                )
            });

        let lift = motion::running(PROFILES_PULSE, cx).unwrap_or(0.0);
        let lift_y = motion::keyframes(&[(0.0, 0.0), (0.4, -2.0), (1.0, 0.0)], lift, motion::EASE);
        let lift_scale =
            motion::keyframes(&[(0.0, 1.0), (0.4, 1.08), (1.0, 1.0)], lift, motion::EASE);

        let pop = motion::running(THEME_POP, cx).unwrap_or(0.0);
        let pop_rotate = motion::keyframes(&POP_ROTATE, pop, POP_CURVE);
        let pop_scale = motion::keyframes(&POP_SCALES, pop, POP_CURVE);
        let swap = motion::value(THEME_SWAP, cx);
        let theme_color = hover_color("theme-btn", cx);
        let swapped = |rest: f32, turn: f32, opacity: f32, glyph: Icon| {
            let scale = pop_scale * (0.4 + 0.6 * rest);
            icon(glyph, px(17.0), theme_color)
                .absolute()
                .opacity(opacity)
                .with_transformation(
                    Transformation::rotate(Radians((pop_rotate + turn).to_radians()))
                        .with_scaling(size(scale, scale)),
                )
        };
        let theme_icon = div()
            .relative()
            .size(px(17.0))
            .child(swapped(1.0 - swap, 90.0 * swap, 1.0 - swap, Icon::Sun))
            .child(swapped(swap, -90.0 * (1.0 - swap), swap, Icon::Moon));

        let swing = motion::running(BELL_SWING, cx);
        let bell_angle = match swing {
            Some(t) => motion::keyframes(&SWING, t, SWING_CURVE),
            None if has_update => ambient::cycle(Duration::from_secs(10), cx)
                .map_or(0.0, |t| motion::keyframes(&RING, t, motion::EASE_IN_OUT)),
            None => 0.0,
        };
        let pivot = 0.36 * 18.0;
        let radians = bell_angle.to_radians();
        let bell_transform = Transformation::rotate(Radians(radians)).with_translation(point(
            px(-pivot * radians.sin()),
            px(pivot * radians.cos() - pivot),
        ));

        let buttons = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                IconButton::new("stats-btn", Key::of("stats-btn"), IconStyle::Round)
                    .tooltip("Статистика")
                    .content(stats_icon)
                    .hover_track(
                        STATS_PULSE,
                        Duration::from_secs_f32(STATS_ENTER),
                        Duration::from_secs_f32(STATS_BAR),
                    )
                    .sound(Some(Sound::Stats))
                    .on_click(cx.listener(|this, _, window, cx| this.open_stats(window, cx))),
            )
            .child(
                IconButton::new("logs-btn", Key::of("logs-btn"), IconStyle::Round)
                    .content(logs_icon)
                    .tooltip("Логи запущенных клиентов")
                    .sound(Some(Sound::Logs))
                    .on_click(cx.listener(|this, _, window, cx| this.open_logs(window, cx))),
            )
            .child(
                round("profiles-btn", Icon::Layers, 17.0, "Профили настроек")
                    .glyph_transform(
                        Transformation::scale(size(lift_scale, lift_scale))
                            .with_translation(point(px(0.0), px(lift_y))),
                    )
                    .pulse(PROFILES_PULSE, Duration::from_millis(500))
                    .sound(Some(Sound::Profiles))
                    .on_click(cx.listener(|this, _, window, cx| this.open_profiles(window, cx))),
            )
            .child(
                IconButton::new("theme-btn", Key::of("theme-btn"), IconStyle::Round)
                    .content(theme_icon)
                    .pulse(THEME_POP, Duration::from_millis(600))
                    .tooltip(theme_tip)
                    .sound(None)
                    .on_click(cx.listener(|this, event: &gpui::ClickEvent, window, cx| {
                        this.toggle_theme(event.position(), window, cx);
                        let light = this.store.read(cx).settings.theme == Theme::Light;
                        motion::drive(
                            THEME_SWAP,
                            light,
                            Duration::from_millis(320),
                            motion::EASE_OUT,
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                round("settings-btn", Icon::Gear, 17.0, "Настройки")
                    .rotate_on_hover(60.0, Duration::from_millis(400), motion::EASE)
                    .sound(Some(Sound::Gear))
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            )
            .child(
                div()
                    .relative()
                    .child(
                        IconButton::new("bell-btn", Key::of("bell-btn"), IconStyle::Round)
                            .icon(Icon::Bell, 18.0)
                            .glyph_transform(bell_transform)
                            .pulse(BELL_SWING, Duration::from_millis(800))
                            .active(bell_active)
                            .tooltip("Уведомления")
                            .sound(Some(Sound::Bell))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Bell, cx)),
                            ),
                    )
                    .when(has_update, |bell| {
                        bell.child(
                            div()
                                .absolute()
                                .top(px(-3.0))
                                .right(px(-3.0))
                                .size(px(10.0))
                                .rounded_full()
                                .bg(p.blue)
                                .border_2()
                                .border_color(p.graphite),
                        )
                    })
                    .when(bell_open, |bell| bell.child(self.render_notifications(cx))),
            );

        let close_tip = if tray {
            "Свернуть в трей"
        } else {
            "Свернуть — приложение продолжит ловить токены в фоне"
        };
        let controls = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .ml(px(10.0))
            .child(
                div().window_control_area(WindowControlArea::Min).child(
                    IconButton::new("win-min", Key::of("win-min"), IconStyle::Window)
                        .icon(Icon::WinMin, 12.0)
                        .tooltip("Свернуть")
                        .on_click(|_, window, _| window.minimize_window()),
                ),
            )
            .child(
                div().window_control_area(WindowControlArea::Max).child(
                    IconButton::new("win-max", Key::of("win-max"), IconStyle::Window)
                        .icon(
                            if maximized {
                                Icon::WinRestore
                            } else {
                                Icon::WinMax
                            },
                            12.0,
                        )
                        .tooltip(if maximized {
                            "Свернуть в окно"
                        } else {
                            "Развернуть"
                        })
                        .on_click(|_, window, _| crate::platform::window::toggle_maximize(window)),
                ),
            )
            .child(
                div().window_control_area(WindowControlArea::Close).child(
                    IconButton::new("win-close", Key::of("win-close"), IconStyle::WindowClose)
                        .icon(Icon::WinClose, 12.0)
                        .tooltip(close_tip)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.close_to_background(window, cx)),
                        ),
                ),
            );

        div()
            .id("topbar")
            .flex_none()
            .h(px(TITLEBAR_HEIGHT + 1.0))
            .w_full()
            .border_b_1()
            .border_color(p.line)
            .bg(p.topbar)
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, |event, window, _| {
                if event.click_count >= 2 {
                    crate::platform::window::toggle_maximize(window);
                } else {
                    crate::platform::window::begin_move(window);
                }
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(if narrow { 48.0 } else { TITLEBAR_HEIGHT }))
                    .px(px(gutter))
                    .child(logo)
                    .when(!narrow, |bar| {
                        bar.child(div().w(px(1.0)).h(px(20.0)).mx(px(14.0)).bg(p.line_strong))
                            .child(brand)
                    })
                    .children(badge)
                    .child(div().flex_1())
                    .child(buttons)
                    .child(controls),
            )
    }

    fn render_online_badge(
        &mut self,
        stats: OnlineStats,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = cx.palette();
        let key = Key::of("active-badge");
        let hover = motion::value(key, cx);
        let (live, count, tip) = match stats {
            OnlineStats::Live(counts) => (
                true,
                counts.online,
                "Сейчас онлайн - нажми для подробностей",
            ),
            OnlineStats::Disabled => (false, 0, "Отправка метрик выключена, потому онлайн скрыт"),
            OnlineStats::Unavailable => (false, 0, "Онлайн временно недоступен"),
        };
        let tone = if live { p.green } else { rgb(0x828282) };
        let (bg, border) = if live {
            (0.14 + 0.08 * hover, 0.25 + 0.15 * hover)
        } else {
            (0.12 + 0.08 * hover, 0.28 + 0.12 * hover)
        };
        let dot_color = if live { p.online } else { rgb(0x8a8a8a) };
        let open = self.popup.as_ref() == Some(&Popup::Online);
        let pulse = live
            .then(|| ambient::cycle(Duration::from_secs(2), cx))
            .flatten()
            .map(|t| {
                let k = (t / 0.7).min(1.0);
                (6.0 * k, 0.55 * (1.0 - k))
            });
        let badge = div()
            .id("active-badge")
            .relative()
            .flex()
            .items_center()
            .gap(px(6.0))
            .ml(px(8.0))
            .px(px(9.0))
            .py(px(3.0))
            .rounded_full()
            .bg(alpha(tone, bg))
            .border_1()
            .border_color(alpha(tone, border))
            .cursor_pointer()
            .occlude()
            .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                sound::play(Sound::Tap);
                this.toggle_popup(Popup::Online, cx);
            }))
            .child(
                div()
                    .text_size(rem(0.74))
                    .font(crate::theme::tabular(FontWeight::BOLD))
                    .text_color(p.text)
                    .child(count.to_string()),
            )
            .child(
                div()
                    .relative()
                    .size(px(7.0))
                    .when_some(pulse, |dot, (grow, fade)| {
                        dot.child(
                            div()
                                .absolute()
                                .top(px(-grow))
                                .left(px(-grow))
                                .size(px(7.0 + grow * 2.0))
                                .rounded_full()
                                .bg(alpha(dot_color, fade)),
                        )
                    })
                    .child(div().absolute().size(px(7.0)).rounded_full().bg(dot_color)),
            )
            .when(open, |badge| badge.child(self.render_online_pop(stats, cx)));
        let badge = if open {
            badge.into_any_element()
        } else {
            with_tooltip(badge, key, tip.into()).into_any_element()
        };
        div()
            .child(badge)
            .with_animation(
                "active-badge-in",
                gpui::Animation::new(Duration::from_millis(200))
                    .with_easing(motion::eased(motion::EASE)),
                |badge, t| badge.opacity(t).mt(px(-4.0 * (1.0 - t))),
            )
            .into_any_element()
    }
}
