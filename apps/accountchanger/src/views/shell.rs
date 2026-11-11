use std::sync::Arc;
use std::time::{Duration, Instant};

use ac_api::Theme;
use gpui::{
    AnimationExt, AnyElement, App, Bounds, Context, Corners, Entity, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Point,
    Render, RenderImage, SharedString, StatefulInteractiveElement, Styled, StyledImage,
    Subscription, Task, Window, canvas, deferred, div, img, point, prelude::FluentBuilder, px,
    relative, size,
};

use crate::ambient;
use crate::background::{self, Backdrop};
use crate::motion;
use crate::sound::{self, Sound};
use crate::state::{Store, StoreEvent};
use crate::theme::{
    GUTTER, GUTTER_COMPACT, GUTTER_NARROW, LINE_HEIGHT, ThemeExt, rem, rgba, shadow,
};
use crate::ui::{
    MenuItem, Tooltips, drop_in, menu_item, menu_separator, menu_surface, tooltip_overlay,
};
use crate::video::VideoPlayer;
use crate::views::account::AccountView;
use crate::views::dialogs::{ConfirmView, PromptView};
use crate::views::groups::{AddMembersView, GroupsView, LaunchSettingsView};
use crate::views::home::Home;
use crate::views::import::{CredEntryView, ImportView};
use crate::views::logs::LogsView;
use crate::views::profiles::{ProfileImportView, ProfilesView};
use crate::views::settings::SettingsView;
use crate::views::stats::StatsView;
use futures::StreamExt;

const VIDEO_NOTICE: Duration = Duration::from_millis(600);
const WAVE: Duration = Duration::from_millis(550);
const DISMISS_GRACE: Duration = Duration::from_millis(250);
const DROP_OUT: Duration = Duration::from_millis(120);
const MODAL_OUT: Duration = Duration::from_millis(180);
const TOAST_OUT: Duration = Duration::from_millis(180);

#[derive(Clone, Debug, PartialEq)]
pub enum Popup {
    GroupMenu,
    Filter,
    Bell,
    Online,
    Queue,
    Dropdown(&'static str),
    Card {
        uuid: String,
        x: f32,
        y: f32,
    },
    LogSession {
        session: String,
        label: String,
        x: f32,
        y: f32,
    },
    Selection {
        x: f32,
        y: f32,
        full: bool,
    },
}

impl Popup {
    fn exits(&self) -> bool {
        matches!(
            self,
            Self::GroupMenu | Self::Filter | Self::Bell | Self::Queue | Self::Dropdown(_)
        )
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::GroupMenu => "group",
            Self::Filter => "filter",
            Self::Bell => "bell",
            Self::Online => "online",
            Self::Queue => "queue",
            Self::Dropdown(id) => id,
            Self::Card { .. } => "card",
            Self::LogSession { .. } => "log-session",
            Self::Selection { .. } => "selection",
        }
    }
}

pub enum Overlay {
    Account(AccountView),
    Settings(SettingsView),
    Logs(LogsView),
    Stats(StatsView),
    Profiles(ProfilesView),
    ProfileImport(ProfileImportView),
    Groups(GroupsView),
    AddMembers(AddMembersView),
    LaunchSettings(LaunchSettingsView),
    Import(ImportView),
    CredEntry(CredEntryView),
    Prompt(PromptView),
    Confirm(ConfirmView),
}

const WAVE_TILE: u32 = 64;
const WAVE_MARGIN: f32 = 0.01;

struct WaveTile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    image: Arc<RenderImage>,
    edge: Option<Arc<RenderImage>>,
}

struct ThemeWave {
    origin: Point<Pixels>,
    started: Instant,
    width: u32,
    height: u32,
    tiles: Vec<WaveTile>,
}

enum Coverage {
    Kept,
    Edge,
    Gone,
}

fn tile_image(width: u32, height: u32, pixels: Vec<u8>) -> Option<Arc<RenderImage>> {
    let frame = image::RgbaImage::from_raw(width, height, pixels)?;
    Some(Arc::new(RenderImage::new(smallvec::smallvec![
        image::Frame::new(frame)
    ])))
}

impl WaveTile {
    fn coverage(&self, (cx, cy): (f32, f32), radius: f32) -> Coverage {
        let (left, right) = (self.x as f32 + 0.5, (self.x + self.width) as f32 - 0.5);
        let (top, bottom) = (self.y as f32 + 0.5, (self.y + self.height) as f32 - 0.5);
        let near = (cx.clamp(left, right) - cx).hypot(cy.clamp(top, bottom) - cy);
        if near >= radius + 0.5 + WAVE_MARGIN {
            return Coverage::Kept;
        }
        let far = (cx - left)
            .abs()
            .max((right - cx).abs())
            .hypot((cy - top).abs().max((bottom - cy).abs()));
        if far <= radius - 0.5 - WAVE_MARGIN {
            Coverage::Gone
        } else {
            Coverage::Edge
        }
    }

    fn carve(&self, (cx, cy): (f32, f32), radius: f32) -> Option<Arc<RenderImage>> {
        let source = self.image.as_bytes(0)?;
        let mut pixels = source.to_vec();
        for row in 0..self.height {
            let dy = (self.y + row) as f32 + 0.5 - cy;
            for column in 0..self.width {
                let dx = (self.x + column) as f32 + 0.5 - cx;
                let keep = ((dx * dx + dy * dy).sqrt() - radius + 0.5).clamp(0.0, 1.0);
                if keep >= 1.0 {
                    continue;
                }
                let index = ((row * self.width + column) * 4) as usize;
                let pixel = &mut pixels[index..index + 4];
                if keep <= 0.0 {
                    pixel.fill(0);
                } else {
                    for channel in 0..3 {
                        pixel[channel] = (f32::from(source[index + channel]) * keep).round() as u8;
                    }
                    pixel[3] = (255.0 * keep).round() as u8;
                }
            }
        }
        tile_image(self.width, self.height, pixels)
    }
}

impl ThemeWave {
    fn new(origin: Point<Pixels>, snapshot: crate::platform::window::Snapshot) -> Option<Self> {
        let (width, height) = (snapshot.width, snapshot.height);
        let mut tiles = Vec::new();
        for y in (0..height).step_by(WAVE_TILE as usize) {
            let rows = WAVE_TILE.min(height - y);
            for x in (0..width).step_by(WAVE_TILE as usize) {
                let columns = WAVE_TILE.min(width - x);
                let stride = (columns * 4) as usize;
                let mut pixels = Vec::with_capacity(stride * rows as usize);
                for row in y..y + rows {
                    let start = ((row * width + x) * 4) as usize;
                    pixels.extend_from_slice(&snapshot.pixels[start..start + stride]);
                }
                tiles.push(WaveTile {
                    x,
                    y,
                    width: columns,
                    height: rows,
                    image: tile_image(columns, rows, pixels)?,
                    edge: None,
                });
            }
        }
        Some(Self {
            origin,
            started: Instant::now(),
            width,
            height,
            tiles,
        })
    }

    fn release(self, window: &mut Window) {
        for tile in self.tiles {
            window.drop_image(tile.image).ok();
            if let Some(edge) = tile.edge {
                window.drop_image(edge).ok();
            }
        }
    }
}

struct ShownToast {
    id: u64,
    text: SharedString,
    error: bool,
    hidden: Option<Instant>,
}

pub struct Shell {
    pub store: Entity<Store>,
    pub home: Home,
    pub overlays: Vec<Overlay>,
    pub popup: Option<Popup>,
    closing: Option<(Popup, Instant)>,
    shown_toast: Option<ShownToast>,
    leaving: Vec<(Overlay, Instant)>,
    dismissed: Option<(&'static str, Instant)>,
    pub refreshing: bool,
    pub update_started: bool,
    pub backdrop: Backdrop,
    focus: FocusHandle,
    font: SharedString,
    ambient_task: Option<Task<()>>,
    theme_wave: Option<ThemeWave>,
    pub(crate) cabinet_login: Option<crate::views::cabinet::CabinetLogin>,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Shell {
    pub fn new(
        store: Entity<Store>,
        font: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = vec![
            cx.observe_in(&store, window, |shell, store, window, cx| {
                shell.sync_backdrop(&store, cx);
                shell.sync_group_order(cx);
                shell.finish_cancelled_import(window, cx);
                cx.notify();
            }),
            cx.subscribe_in(&store, window, |shell, _, event, window, cx| match event {
                StoreEvent::Focus => shell.reveal(window, cx),
                StoreEvent::LogsChanged => shell.on_logs_changed(cx),
                StoreEvent::Quit => cx.quit(),
            }),
        ];
        subscriptions.push(cx.observe_window_activation(window, |shell, window, cx| {
            shell.update_ambient(window, cx);
        }));
        let home = Home::new(window, cx);
        Self {
            store,
            home,
            overlays: Vec::new(),
            popup: None,
            closing: None,
            shown_toast: None,
            leaving: Vec::new(),
            dismissed: None,
            refreshing: false,
            update_started: false,
            backdrop: Backdrop::default(),
            focus: cx.focus_handle(),
            font,
            ambient_task: None,
            theme_wave: None,
            cabinet_login: None,
            _subscriptions: subscriptions,
        }
    }

    fn update_ambient(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let live = window.is_window_active();
        ambient::set_live(live, cx);
        self.backdrop.set_live(live);
        if live && self.ambient_task.is_none() {
            self.ambient_task = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(ambient::FRAME).await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            }));
        } else if !live {
            self.ambient_task = None;
        }
        cx.notify();
    }

    fn sync_backdrop(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let settings = store.read(cx).settings.background.clone();
        if let Some(load) = self.backdrop.plan(&settings) {
            if ac_api::is_video(&load.file) {
                self.load_video(load, store, cx);
                return;
            }
            let future = store.read(cx).backend.background_image();
            cx.spawn(async move |this, cx| {
                let bytes = future.await.ok().flatten();
                let image = match bytes {
                    Some(bytes) => {
                        cx.background_executor()
                            .spawn(async move { background::prepare(&bytes, load.blur) })
                            .await
                    }
                    None => None,
                };
                this.update(cx, |shell, cx| {
                    shell.backdrop.finish(load, image);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    fn load_video(
        &mut self,
        load: background::Load,
        store: &Entity<Store>,
        cx: &mut Context<Self>,
    ) {
        let future = store.read(cx).backend.background_video();
        let store = store.clone();
        cx.spawn(async move |this, cx| {
            let waiting = Arc::new(std::sync::atomic::AtomicBool::new(true));
            let notice = waiting.clone();
            let toast_store = store.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(VIDEO_NOTICE).await;
                if notice.load(std::sync::atomic::Ordering::Relaxed) {
                    toast_store
                        .update(cx, |store, cx| {
                            store.toast("Скачиваю FFmpeg для видео-фона (~30 МБ)…", false, cx)
                        })
                        .ok();
                }
            })
            .detach();
            let result = future.await;
            waiting.store(false, std::sync::atomic::Ordering::Relaxed);
            let started = match result {
                Ok(Some(source)) => match VideoPlayer::start(&source, load.blur) {
                    Ok(started) => Some(started),
                    Err(error) => {
                        store
                            .update(cx, |store, cx| {
                                store.toast(
                                    format!("Не удалось запустить видео: {error}"),
                                    true,
                                    cx,
                                )
                            })
                            .ok();
                        None
                    }
                },
                Ok(None) => None,
                Err(error) => {
                    store
                        .update(cx, |store, cx| {
                            store.toast(error.message().to_owned(), true, cx)
                        })
                        .ok();
                    None
                }
            };
            let (player, mut frames) = match started {
                Some((player, frames)) => (Some(player), Some(frames)),
                None => (None, None),
            };
            let playing = this
                .update(cx, |shell, cx| {
                    let playing = shell.backdrop.finish_video(load, player);
                    shell.backdrop.set_live(ambient::is_live(cx));
                    cx.notify();
                    playing
                })
                .unwrap_or(false);
            let Some(frames) = frames.as_mut().filter(|_| playing) else {
                return;
            };
            while let Some(frame) = frames.next().await {
                let alive = this.update(cx, |shell, cx| {
                    shell.backdrop.push_frame(frame);
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::platform::window::restore(window);
        self.update_ambient(window, cx);
    }

    pub(crate) fn close_to_background(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Tooltips::dismiss(cx);
        self.close_cabinet_login(cx);
        crate::platform::window::hide(window);
        ambient::set_live(false, cx);
        self.backdrop.set_live(false);
        self.ambient_task = None;
    }

    pub(crate) fn gutter(&self, window: &Window) -> f32 {
        let width = f32::from(window.viewport_size().width);
        if width <= crate::theme::NARROW_WIDTH {
            GUTTER_NARROW
        } else if width <= crate::theme::COMPACT_WIDTH {
            GUTTER_COMPACT
        } else {
            GUTTER
        }
    }

    pub(crate) fn toast(&self, text: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        let text = text.into();
        self.store
            .update(cx, |store, cx| store.toast(text, error, cx));
    }

    pub(crate) fn toggle_popup(&mut self, popup: Popup, cx: &mut Context<Self>) {
        let kind = popup.kind();
        let just_dismissed = self
            .dismissed
            .take()
            .is_some_and(|(dismissed, at)| dismissed == kind && at.elapsed() < DISMISS_GRACE);
        if self.popup.as_ref().is_some_and(|open| open.kind() == kind) || just_dismissed {
            self.set_popup(None);
        } else {
            self.set_popup(Some(popup));
        }
        Tooltips::dismiss(cx);
        cx.notify();
    }

    pub(crate) fn dismiss_popup(&mut self, popup: Popup, cx: &mut Context<Self>) {
        if self
            .popup
            .as_ref()
            .is_some_and(|open| open.kind() == popup.kind())
        {
            self.set_popup(None);
            self.dismissed = Some((popup.kind(), Instant::now()));
            cx.notify();
        }
    }

    pub(crate) fn close_popup(&mut self, cx: &mut Context<Self>) {
        if self.popup.is_some() {
            self.set_popup(None);
            cx.notify();
        }
    }

    pub(crate) fn set_popup(&mut self, popup: Option<Popup>) {
        if let Some(previous) = std::mem::replace(&mut self.popup, popup)
            && previous.exits()
        {
            self.closing = Some((previous, Instant::now()));
        }
    }

    pub(crate) fn popup_shown(&self, popup: &Popup) -> Option<Option<f32>> {
        if self
            .popup
            .as_ref()
            .is_some_and(|open| open.kind() == popup.kind())
        {
            return Some(None);
        }
        let (closing, at) = self.closing.as_ref()?;
        let progress = at.elapsed().as_secs_f32() / DROP_OUT.as_secs_f32();
        (closing.kind() == popup.kind() && progress < 1.0).then_some(Some(progress))
    }

    pub(crate) fn retire(&mut self, overlay: Overlay, window: &mut Window, cx: &mut Context<Self>) {
        motion::release_hovers(cx);
        Tooltips::dismiss(cx);
        self.focus.focus(window);
        self.leaving.push((overlay, Instant::now()));
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(MODAL_OUT).await;
            this.update_in(cx, |shell, window, cx| shell.sweep_leaving(window, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn sweep_leaving(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.leaving)
            .into_iter()
            .partition(|(_, at)| at.elapsed() >= MODAL_OUT);
        self.leaving = kept;
        for (overlay, _) in gone {
            self.on_overlay_closed(overlay, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn close_card_menu(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.popup,
            Some(Popup::Card { .. } | Popup::LogSession { .. } | Popup::Selection { .. })
        ) {
            self.set_popup(None);
            cx.notify();
        }
    }

    pub(crate) fn push_overlay(&mut self, overlay: Overlay, cx: &mut Context<Self>) {
        self.set_popup(None);
        Tooltips::dismiss(cx);
        motion::release_hovers(cx);
        self.overlays.push(overlay);
        cx.notify();
    }

    fn on_overlay_closed(&mut self, overlay: Overlay, window: &mut Window, cx: &mut Context<Self>) {
        match overlay {
            Overlay::Prompt(mut prompt) => prompt.resolve(None, self, window, cx),
            Overlay::Confirm(mut confirm) => confirm.resolve(false, self, window, cx),
            Overlay::Account(mut account) => {
                account.flush_label(self, cx);
                if let Some(frame) = account.stage.frame.take() {
                    cx.drop_image(frame, Some(window));
                }
            }
            _ => {}
        }
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_cabinet_login(cx) {
            return;
        }
        if self.popup.is_some() {
            self.set_popup(None);
            cx.notify();
            return;
        }
        if let Some(overlay) = self.overlays.pop() {
            self.retire(overlay, window, cx);
            return;
        }
        self.clear_selection(cx);
    }

    pub(crate) fn toggle_theme(
        &mut self,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .theme_wave
            .as_ref()
            .is_some_and(|wave| wave.started.elapsed() < WAVE)
        {
            return;
        }
        let theme = self.store.read(cx).settings.theme.toggled();
        sound::play(if theme == Theme::Light {
            Sound::ThemeLight
        } else {
            Sound::ThemeDark
        });
        if let Some(previous) = self.theme_wave.take() {
            previous.release(window);
        }
        self.theme_wave = crate::platform::window::snapshot(window)
            .and_then(|snapshot| ThemeWave::new(origin, snapshot));
        crate::theme::install(theme, cx);
        self.store.update(cx, |store, cx| {
            store.settings.theme = theme;
            let future = store.backend.set_theme(theme);
            store.act(cx, future, "");
        });
        cx.refresh_windows();
    }

    fn render_theme_wave(&mut self, window: &mut Window) -> Option<AnyElement> {
        let wave = self.theme_wave.as_mut()?;
        let elapsed = wave.started.elapsed();
        if elapsed >= WAVE {
            if let Some(wave) = self.theme_wave.take() {
                wave.release(window);
            }
            return None;
        }
        window.request_animation_frame();
        let scale = window.scale_factor();
        let viewport = window.viewport_size();
        let (x, y) = (f32::from(wave.origin.x), f32::from(wave.origin.y));
        let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
        let reach = x.max(width - x).hypot(y.max(height - y));
        let progress =
            motion::cubic_bezier(motion::STANDARD, elapsed.as_secs_f32() / WAVE.as_secs_f32());
        let center = (x * scale, y * scale);
        let radius = reach * progress * scale;
        let mut visible = Vec::with_capacity(wave.tiles.len());
        for tile in &mut wave.tiles {
            let coverage = tile.coverage(center, radius);
            let edge = match coverage {
                Coverage::Edge => tile.carve(center, radius),
                Coverage::Kept | Coverage::Gone => None,
            };
            if let Some(previous) = std::mem::replace(&mut tile.edge, edge) {
                window.drop_image(previous).ok();
            }
            let shown = match coverage {
                Coverage::Kept => Some(tile.image.clone()),
                Coverage::Edge => tile.edge.clone(),
                Coverage::Gone => None,
            };
            if let Some(image) = shown {
                visible.push((tile.x, tile.y, tile.width, tile.height, image));
            }
        }
        let (columns, rows) = (wave.width as f32, wave.height as f32);
        Some(
            deferred(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        let scale = window.scale_factor();
                        let across = f32::from(bounds.size.width) * scale / columns;
                        let down = f32::from(bounds.size.height) * scale / rows;
                        for (x, y, width, height, image) in visible {
                            let left = (x as f32 * across).round();
                            let top = (y as f32 * down).round();
                            let right = ((x + width) as f32 * across).round();
                            let bottom = ((y + height) as f32 * down).round();
                            let tile = Bounds::new(
                                point(
                                    bounds.origin.x + px((left + 0.25) / scale),
                                    bounds.origin.y + px((top + 0.25) / scale),
                                ),
                                size(
                                    px((right - left - 0.25) / scale),
                                    px((bottom - top - 0.25) / scale),
                                ),
                            );
                            window
                                .paint_image(tile, Corners::default(), image, 0, false)
                                .ok();
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .with_priority(20)
            .into_any_element(),
        )
    }

    pub(crate) fn open_launcher(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            let future = store.backend.open_launcher();
            store.act(cx, future, "Открываю лаунчер…");
        });
    }

    pub(crate) fn launch_guest(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            let future = store.backend.launch_guest();
            store.act(cx, future, "Запускаю лаунчер без аккаунта…");
        });
    }

    pub(crate) fn refresh_tokens(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        cx.notify();
        let future = self.store.read(cx).backend.refresh_start();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |shell, cx| {
                shell.refreshing = false;
                let message = match result {
                    Ok(()) => (
                        "Обновляю истекающие токены - продолжит в фоне".to_owned(),
                        false,
                    ),
                    Err(error) => (error.message().to_owned(), true),
                };
                store.update(cx, |store, cx| store.toast(message.0, message.1, cx));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.key == "escape" {
            self.escape(window, cx);
            cx.stop_propagation();
        } else if keystroke.modifiers.control
            && !keystroke.modifiers.shift
            && (keystroke.key == "c" || keystroke.key == "a")
            && self.copy_log_selection(keystroke.key == "a", cx)
        {
            cx.stop_propagation();
        } else if keystroke.modifiers.control
            && !keystroke.modifiers.shift
            && keystroke.key == "a"
            && self.overlays.is_empty()
            && !self.home.search.read(cx).is_focused(window)
        {
            self.select_all(cx);
            cx.stop_propagation();
        } else if keystroke.key == "delete"
            && self.overlays.is_empty()
            && !self.home.search.read(cx).is_focused(window)
            && self.forget_selected(window, cx)
        {
            cx.stop_propagation();
        } else if keystroke.key == "e" && keystroke.modifiers.control && keystroke.modifiers.shift {
            self.home.debug_expired = !self.home.debug_expired;
            let message = if self.home.debug_expired {
                "Тест: все токены «истёк»"
            } else {
                "Тест выключен"
            };
            self.toast(message, false, cx);
            cx.notify();
        }
    }

    fn render_card_menu(
        &mut self,
        uuid: &str,
        x: f32,
        y: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let p = cx.palette();
        let store = self.store.read(cx);
        let account = store.account(uuid)?.clone();
        let group = self.active_group(store).cloned();
        let mut items: Vec<(SharedString, MenuItem, CardAction)> = vec![(
            "Информация об аккаунте".into(),
            MenuItem::Plain,
            CardAction::Info,
        )];
        if account.running {
            items.push(("Закрыть клиент".into(), MenuItem::Plain, CardAction::Toggle));
        } else if account.launching {
            items.push((
                "Остановить запуск".into(),
                MenuItem::Plain,
                CardAction::StopLaunch,
            ));
        } else if account.launchable {
            items.push(("Запустить".into(), MenuItem::Plain, CardAction::Toggle));
        }
        items.push((
            if account.pinned {
                "Открепить"
            } else {
                "Закрепить"
            }
            .into(),
            MenuItem::Plain,
            CardAction::Pin,
        ));
        let mut separators = Vec::new();
        if let Some(group) = &group {
            separators.push(items.len());
            items.push((
                format!("Убрать из «{}»", group.name).into(),
                MenuItem::Danger,
                CardAction::RemoveFromGroup(group.id.clone()),
            ));
        }
        separators.push(items.len());
        items.push((
            "Удалить аккаунт".into(),
            MenuItem::Danger,
            CardAction::Forget,
        ));

        let item_height = 13.6 * LINE_HEIGHT + 18.0;
        let height =
            items.len() as f32 * (item_height + 2.0) + separators.len() as f32 * 9.0 + 14.0;
        let width = 220.0;
        let viewport = window.viewport_size();
        let left = x.min(f32::from(viewport.width) - width - 8.0).max(8.0);
        let top = y.min(f32::from(viewport.height) - height - 8.0).max(8.0);

        let mut menu = menu_surface(p.surface, 11.0, 6.0, cx)
            .gap(px(2.0))
            .min_w(px(190.0))
            .text_size(rem(0.85));
        for (index, (label, kind, action)) in items.into_iter().enumerate() {
            if separators.contains(&index) {
                menu = menu.child(menu_separator(&p, 6.0, 4.0));
            }
            let target = account.uuid.clone();
            menu = menu.child(
                menu_item(index, kind, &p)
                    .px(px(12.0))
                    .py(px(9.0))
                    .rounded(px(7.0))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        sound::play(Sound::Tap);
                        this.popup = None;
                        this.run_card_action(&target, action.clone(), window, cx);
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        Some(
            deferred(
                div().absolute().left(px(left)).top(px(top)).child(drop_in(
                    "card-menu-in",
                    menu.id("card-menu")
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_card_menu(cx))),
                    None,
                )),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }

    fn run_card_action(
        &mut self,
        uuid: &str,
        action: CardAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            CardAction::Info => self.open_account(uuid.to_owned(), window, cx),
            CardAction::Toggle => self
                .store
                .update(cx, |store, cx| store.toggle_play(uuid, cx)),
            CardAction::StopLaunch => self
                .store
                .update(cx, |store, cx| store.stop_launch(uuid, cx)),
            CardAction::Pin => self
                .store
                .update(cx, |store, cx| store.toggle_pin(uuid, cx)),
            CardAction::RemoveFromGroup(group) => self.store.update(cx, |store, cx| {
                let future = store.backend.remove_group_member(group, uuid.to_owned());
                store.act(cx, future, "");
            }),
            CardAction::Forget => self.confirm_forget(uuid.to_owned(), false, window, cx),
        }
    }

    pub(crate) fn confirm_forget(
        &mut self,
        uuid: String,
        from_modal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(account) = self.store.read(cx).account(&uuid).cloned() else {
            return;
        };
        let text = format!(
            "Аккаунт «{}» будет убран из списка.",
            account.display_name()
        );
        self.confirm(
            "Удалить аккаунт?",
            text,
            "Да, удалить",
            move |this, window, cx| {
                let future = this.store.read(cx).backend.forget(uuid.clone());
                this.store.update(cx, |store, cx| {
                    store.act(
                        cx,
                        future,
                        if from_modal {
                            "Аккаунт убран из списка"
                        } else {
                            ""
                        },
                    );
                });
                if from_modal {
                    this.close::<AccountView>(window, cx);
                }
            },
            window,
            cx,
        );
    }

    fn render_toast(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = cx.palette();
        let fade = match &self.store.read(cx).toast {
            Some(toast) => {
                if self.shown_toast.as_ref().is_none_or(|shown| {
                    shown.id != toast.id
                        || shown.hidden.is_some()
                        || shown.text.as_ref() != toast.text.as_str()
                }) {
                    self.shown_toast = Some(ShownToast {
                        id: toast.id,
                        text: toast.text.clone().into(),
                        error: toast.error,
                        hidden: None,
                    });
                }
                None
            }
            None => {
                let shown = self.shown_toast.as_mut()?;
                let hidden = *shown.hidden.get_or_insert_with(Instant::now);
                let t = hidden.elapsed().as_secs_f32() / TOAST_OUT.as_secs_f32();
                if t >= 1.0 {
                    self.shown_toast = None;
                    return None;
                }
                window.request_animation_frame();
                Some(motion::cubic_bezier(motion::EASE, t))
            }
        };
        let toast = self.shown_toast.as_ref()?;
        let toast_id = gpui::ElementId::NamedInteger("toast".into(), toast.id);
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(26.0))
                .flex()
                .justify_center()
                .child(
                    div()
                        .max_w(px(440.0))
                        .mx(px(18.0))
                        .px(px(18.0))
                        .py(px(11.0))
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(if toast.error {
                            rgba(0xe0564f, 0.5)
                        } else {
                            p.line_strong
                        })
                        .bg(p.toast)
                        .shadow(shadow(
                            0.0,
                            12.0,
                            30.0,
                            0.0,
                            rgba(0x000000, if p.is_light() { 0.18 } else { 0.5 }),
                        ))
                        .text_color(if toast.error { p.toast_error } else { p.text })
                        .text_size(rem(0.84))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(relative(1.4))
                        .text_center()
                        .child(toast.text.clone())
                        .map(|body| match fade {
                            Some(t) => body.opacity(1.0 - t).mb(px(-10.0 * t)).into_any_element(),
                            None => body
                                .with_animation(
                                    toast_id,
                                    gpui::Animation::new(TOAST_OUT)
                                        .with_easing(motion::eased(motion::EASE)),
                                    |toast, t| toast.opacity(t).mb(px(-10.0 * (1.0 - t))),
                                )
                                .into_any_element(),
                        }),
                )
                .into_any_element(),
        )
    }

    fn render_overlays(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut leaving = std::mem::take(&mut self.leaving);
        let mut rendered: Vec<AnyElement> = leaving
            .iter_mut()
            .map(|(overlay, at)| {
                let t = (at.elapsed().as_secs_f32() / MODAL_OUT.as_secs_f32()).min(1.0);
                let fade = 1.0 - motion::cubic_bezier(motion::EASE, t);
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .opacity(fade)
                    .occlude()
                    .child(self.render_overlay(overlay, window, cx))
                    .into_any_element()
            })
            .collect();
        if !leaving.is_empty() {
            window.request_animation_frame();
        }
        leaving.append(&mut self.leaving);
        self.leaving = leaving;
        let mut overlays = std::mem::take(&mut self.overlays);
        let live: Vec<AnyElement> = overlays
            .iter_mut()
            .map(|overlay| {
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .child(self.render_overlay(overlay, window, cx))
                    .into_any_element()
            })
            .collect();
        rendered.extend(live);
        overlays.append(&mut self.overlays);
        self.overlays = overlays;
        rendered
    }

    fn render_overlay(
        &mut self,
        overlay: &mut Overlay,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match overlay {
            Overlay::Account(view) => view.render(self, window, cx),
            Overlay::Settings(view) => view.render(self, window, cx),
            Overlay::Logs(view) => view.render(self, window, cx),
            Overlay::Stats(view) => view.render(self, window, cx),
            Overlay::Profiles(view) => view.render(self, window, cx),
            Overlay::ProfileImport(view) => view.render(self, window, cx),
            Overlay::Groups(view) => view.render(self, window, cx),
            Overlay::AddMembers(view) => view.render(self, window, cx),
            Overlay::LaunchSettings(view) => view.render(self, window, cx),
            Overlay::Import(view) => view.render(self, window, cx),
            Overlay::CredEntry(view) => view.render(self, window, cx),
            Overlay::Prompt(view) => view.render(self, window, cx),
            Overlay::Confirm(view) => view.render(self, window, cx),
        }
    }
}

pub trait OverlayKind: Sized {
    fn view(overlay: &mut Overlay) -> Option<&mut Self>;
}

macro_rules! overlay_kinds {
    ($($variant:ident => $view:ty),+ $(,)?) => {
        $(impl OverlayKind for $view {
            fn view(overlay: &mut Overlay) -> Option<&mut Self> {
                match overlay {
                    Overlay::$variant(view) => Some(view),
                    _ => None,
                }
            }
        })+
    };
}

overlay_kinds! {
    Account => AccountView,
    Settings => SettingsView,
    Logs => LogsView,
    Stats => StatsView,
    Profiles => ProfilesView,
    ProfileImport => ProfileImportView,
    Groups => GroupsView,
    AddMembers => AddMembersView,
    LaunchSettings => LaunchSettingsView,
    Import => ImportView,
    CredEntry => CredEntryView,
    Prompt => PromptView,
    Confirm => ConfirmView,
}

impl Shell {
    pub(crate) fn overlay<V: OverlayKind>(&mut self) -> Option<&mut V> {
        self.overlays.iter_mut().rev().find_map(V::view)
    }

    pub(crate) fn closer<V: OverlayKind + 'static>(
        cx: &mut Context<Self>,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let listener = cx.listener(|this, _: &(), window, cx| this.close::<V>(window, cx));
        move |window, cx| listener(&(), window, cx)
    }

    pub(crate) fn has_overlay<V: OverlayKind>(&mut self) -> bool {
        self.overlay::<V>().is_some()
    }

    pub(crate) fn close<V: OverlayKind>(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .overlays
            .iter_mut()
            .rposition(|overlay| V::view(overlay).is_some())
        {
            let overlay = self.overlays.remove(index);
            self.retire(overlay, window, cx);
        }
    }
}

#[derive(Clone, Debug)]
enum CardAction {
    Info,
    Toggle,
    StopLaunch,
    Pin,
    RemoveFromGroup(String),
    Forget,
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette();
        motion::keep_alive(window, cx);
        if let Some((_, at)) = &self.closing {
            if at.elapsed() >= DROP_OUT {
                self.closing = None;
            } else {
                window.request_animation_frame();
            }
        }
        for image in self.backdrop.take_retired() {
            window.drop_image(image).ok();
        }
        let backdrop: Option<(Arc<RenderImage>, f32)> = self
            .backdrop
            .image(&self.store.read(cx).settings.background);
        let titlebar = self.render_titlebar(window, cx).into_any_element();
        let home = self.render_home(window, cx).into_any_element();
        let fabs = self.render_fabs(cx).into_any_element();
        let card_menu = match self.popup.clone() {
            Some(Popup::Card { uuid, x, y }) => self.render_card_menu(&uuid, x, y, window, cx),
            Some(Popup::LogSession {
                session,
                label,
                x,
                y,
            }) => self.render_log_session_menu(session, label, x, y, window, cx),
            Some(Popup::Selection { x, y, full }) => {
                self.render_selection_menu(x, y, full, window, cx)
            }
            _ => None,
        };
        let overlays = self.render_overlays(window, cx);
        let toast = self.render_toast(window, cx);
        let tooltip = tooltip_overlay(window, cx).map(|tip| deferred(tip).with_priority(10));
        let wave = self.render_theme_wave(window);
        self.drive_cabinet_login(window, cx);
        let cabinet_login = self.render_cabinet_login(cx);

        div()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(p.graphite)
            .text_color(p.text)
            .font_family(self.font.clone())
            .text_size(px(crate::theme::BASE_TEXT))
            .line_height(relative(LINE_HEIGHT))
            .when_some(backdrop, |root, (image, dim)| {
                root.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .child(img(image).size_full().object_fit(gpui::ObjectFit::Cover))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .bg(rgba(0x000000, dim)),
                        ),
                )
            })
            .child(titlebar)
            .child(home)
            .child(fabs)
            .children(card_menu)
            .children(overlays)
            .children(cabinet_login)
            .children(toast)
            .children(tooltip)
            .children(wave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::window::Snapshot;

    fn punched(original: &[u8], width: usize, center: (f32, f32), radii: &[f32]) -> Vec<u8> {
        let mut pixels = original.to_vec();
        let height = original.len() / 4 / width;
        let (cx, cy) = center;
        for &radius in radii {
            let top = (cy - radius - 1.0).floor().max(0.0) as usize;
            let bottom = ((cy + radius + 1.0).ceil().max(0.0) as usize).min(height);
            let outer = radius + 1.0;
            for y in top..bottom {
                let dy = y as f32 + 0.5 - cy;
                let span = outer * outer - dy * dy;
                if span <= 0.0 {
                    continue;
                }
                let half = span.sqrt();
                let left = (cx - half).floor().max(0.0) as usize;
                let right = ((cx + half).ceil().max(0.0) as usize).min(width);
                for x in left..right {
                    let dx = x as f32 + 0.5 - cx;
                    let keep = ((dx * dx + dy * dy).sqrt() - radius + 0.5).clamp(0.0, 1.0);
                    let index = (y * width + x) * 4;
                    if keep <= 0.0 {
                        pixels[index..index + 4].fill(0);
                    } else if keep < 1.0 {
                        for channel in 0..3 {
                            pixels[index + channel] =
                                (f32::from(original[index + channel]) * keep).round() as u8;
                        }
                        pixels[index + 3] = (255.0 * keep).round() as u8;
                    }
                }
            }
        }
        pixels
    }

    fn composed(wave: &ThemeWave, center: (f32, f32), radius: f32) -> Vec<u8> {
        let width = wave.width as usize;
        let mut pixels = vec![0u8; width * wave.height as usize * 4];
        for tile in &wave.tiles {
            let image = match tile.coverage(center, radius) {
                Coverage::Kept => tile.image.clone(),
                Coverage::Edge => tile.carve(center, radius).unwrap(),
                Coverage::Gone => continue,
            };
            let bytes = image.as_bytes(0).unwrap();
            let stride = tile.width as usize * 4;
            for row in 0..tile.height as usize {
                let start = ((tile.y as usize + row) * width + tile.x as usize) * 4;
                pixels[start..start + stride]
                    .copy_from_slice(&bytes[row * stride..(row + 1) * stride]);
            }
        }
        pixels
    }

    #[test]
    fn tiled_wave_matches_full_frame_punch() {
        let (width, height) = (203usize, 147usize);
        let original: Vec<u8> = (0..width * height * 4)
            .map(|index| {
                if index % 4 == 3 {
                    255
                } else {
                    (index * 37 % 251) as u8
                }
            })
            .collect();
        let wave = ThemeWave::new(
            Point::default(),
            Snapshot {
                width: width as u32,
                height: height as u32,
                pixels: original.clone(),
            },
        )
        .unwrap();
        assert_eq!(wave.tiles.len(), 4 * 3);
        for center in [(150.25, 20.5), (0.0, 0.0), (101.5, 146.75)] {
            let radii: Vec<f32> = (0..=40).map(|step| step as f32 * 6.3).collect();
            for (index, &radius) in radii.iter().enumerate() {
                assert_eq!(
                    composed(&wave, center, radius),
                    punched(&original, width, center, &radii[..=index]),
                );
            }
        }
    }
}
