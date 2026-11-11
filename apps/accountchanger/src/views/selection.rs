use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use ac_api::Account;
use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, ClickEvent, Context, DispatchPhase,
    ElementId, FontWeight, Hsla, InteractiveElement, IntoElement, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, Task, Window, canvas, deferred, div,
    prelude::FluentBuilder, px,
};

use crate::assets::{Icon, icon};
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{LINE_HEIGHT, ThemeExt, mix, rem, rgba, shadow};
use crate::ui::{
    Checkbox, MenuItem, Tooltips, drop_in, menu_item, menu_separator, menu_surface, with_tooltip,
};
use crate::views::shell::{Popup, Shell};

pub const PICK: &str = "card-pick";
pub const PICK_POP: &str = "card-pick-pop";
pub const SELECTING: &str = "selecting";
const PICK_FADE: Duration = Duration::from_millis(170);
const PICK_POP_TIME: Duration = Duration::from_millis(280);
const MARK_FADE: Duration = Duration::from_millis(160);
const DRAG_THRESHOLD: f32 = 5.0;
const EDGE_INSET: f32 = 14.0;
const SCROLL_GAIN: f32 = 0.32;
const SCROLL_MIN: f32 = 2.0;
const SCROLL_MAX: f32 = 44.0;
const SCROLL_TICK: Duration = Duration::from_millis(16);
const PAINT_STEP: f32 = 6.0;
const LABELS_MIN_WIDTH: f32 = 780.0;
const MENU_WIDTH: f32 = 230.0;
const REMOVE_LABEL_WIDTH: f32 = 170.0;
const ALL_LABEL_WIDTH: f32 = 130.0;
const BAR_IN: Duration = Duration::from_millis(180);

pub type CardBounds = Rc<RefCell<HashMap<String, Bounds<Pixels>>>>;

#[derive(Default)]
pub struct Selection {
    pub picked: HashSet<String>,
    pub order: Vec<String>,
    pub cards: CardBounds,
    anchor: Option<String>,
    gesture: Option<Gesture>,
    swallow_click: bool,
    scroll_task: Option<Task<()>>,
}

struct Gesture {
    origin: Point<Pixels>,
    pointer: Point<Pixels>,
    last: Point<Pixels>,
    start: Option<String>,
    additive: bool,
    mode: Mode,
}

enum Mode {
    Pending,
    Marquee(HashSet<String>),
    Paint(bool),
}

#[derive(Clone, Debug)]
enum BulkAction {
    Launch,
    Pin(bool),
    AddTo(String),
    NewGroup,
    RemoveFrom(String),
    Forget,
    Clear,
}

fn content(position: Point<Pixels>, offset: Point<Pixels>) -> Point<Pixels> {
    Point::new(position.x - offset.x, position.y - offset.y)
}

fn span(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Point::new(a.x.max(b.x), a.y.max(b.y)),
    )
}

fn accounts_word(count: usize) -> &'static str {
    let tens = count % 100;
    let ones = count % 10;
    if (11..=14).contains(&tens) {
        "аккаунтов"
    } else if ones == 1 {
        "аккаунт"
    } else if (2..=4).contains(&ones) {
        "аккаунта"
    } else {
        "аккаунтов"
    }
}

fn picked_word(count: usize) -> &'static str {
    if count % 10 == 1 && count % 100 != 11 {
        "Выбран"
    } else {
        "Выбрано"
    }
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.picked.is_empty()
    }

    pub fn contains(&self, uuid: &str) -> bool {
        self.picked.contains(uuid)
    }

    pub fn clear(&mut self) -> bool {
        self.anchor = None;
        if self.picked.is_empty() {
            return false;
        }
        self.picked.clear();
        true
    }

    pub fn dragging(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|gesture| !matches!(gesture.mode, Mode::Pending))
    }

    fn card_at(&self, point: Point<Pixels>) -> Option<String> {
        self.cards
            .borrow()
            .iter()
            .find(|(_, bounds)| bounds.contains(&point))
            .map(|(uuid, _)| uuid.clone())
    }

    fn toggle(&mut self, uuid: &str) {
        if !self.picked.remove(uuid) {
            self.picked.insert(uuid.to_owned());
        }
        self.anchor = Some(uuid.to_owned());
    }

    fn extend_to(&mut self, uuid: &str) {
        let anchor = self.anchor.as_deref().unwrap_or(uuid);
        let from = self.order.iter().position(|item| item == anchor);
        let to = self.order.iter().position(|item| item == uuid);
        match (from, to) {
            (Some(from), Some(to)) => {
                let (low, high) = (from.min(to), from.max(to));
                self.picked.extend(self.order[low..=high].iter().cloned());
            }
            _ => {
                self.picked.insert(uuid.to_owned());
            }
        }
        self.anchor = Some(uuid.to_owned());
    }

    fn track(&mut self, offset: Point<Pixels>) -> bool {
        let Self {
            picked,
            cards,
            gesture,
            ..
        } = self;
        let Some(gesture) = gesture.as_mut() else {
            return false;
        };
        let current = content(gesture.pointer, offset);
        let cards = cards.borrow();
        match &gesture.mode {
            Mode::Pending => false,
            Mode::Marquee(base) => {
                let rect = span(gesture.origin, current);
                let mut next = base.clone();
                next.extend(
                    cards
                        .iter()
                        .filter(|(_, bounds)| bounds.intersects(&rect))
                        .map(|(uuid, _)| uuid.clone()),
                );
                let changed = next != *picked;
                *picked = next;
                changed
            }
            Mode::Paint(target) => {
                let target = *target;
                let from = gesture.last;
                gesture.last = current;
                let dx = f32::from(current.x - from.x);
                let dy = f32::from(current.y - from.y);
                let steps = ((dx.hypot(dy) / PAINT_STEP).ceil() as usize).max(1);
                let mut changed = false;
                for step in 0..=steps {
                    let t = step as f32 / steps as f32;
                    let probe = Point::new(from.x + px(dx * t), from.y + px(dy * t));
                    let Some((uuid, _)) = cards.iter().find(|(_, bounds)| bounds.contains(&probe))
                    else {
                        continue;
                    };
                    changed |= if target {
                        picked.insert(uuid.clone())
                    } else {
                        picked.remove(uuid)
                    };
                }
                changed
            }
        }
    }

    pub fn marquee(&self, offset: Point<Pixels>) -> Option<Bounds<Pixels>> {
        let gesture = self.gesture.as_ref()?;
        matches!(gesture.mode, Mode::Marquee(_)).then(|| {
            let origin = Point::new(gesture.origin.x + offset.x, gesture.origin.y + offset.y);
            span(origin, gesture.pointer)
        })
    }
}

pub fn card_probe(cards: CardBounds, scroll: ScrollHandle, uuid: String) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            let offset = scroll.offset();
            let mut stored = bounds;
            stored.origin = content(bounds.origin, offset);
            cards.borrow_mut().insert(uuid, stored);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl Shell {
    pub(crate) fn selection_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let offset = self.home.scroll.offset();
        let origin = content(event.position, offset);
        let selection = &mut self.home.selection;
        selection.swallow_click = false;
        let start = selection.card_at(origin);
        selection.gesture = Some(Gesture {
            origin,
            pointer: event.position,
            last: origin,
            start,
            additive: event.modifiers.control || event.modifiers.shift,
            mode: Mode::Pending,
        });
        cx.notify();
    }

    fn selection_move(
        &mut self,
        position: Point<Pixels>,
        pressed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !pressed {
            self.selection_up(cx);
            return;
        }
        let offset = self.home.scroll.offset();
        let selection = &mut self.home.selection;
        let Some(gesture) = selection.gesture.as_mut() else {
            return;
        };
        gesture.pointer = position;
        if matches!(gesture.mode, Mode::Pending) {
            let current = content(position, offset);
            let dx = f32::from(current.x - gesture.origin.x);
            let dy = f32::from(current.y - gesture.origin.y);
            if dx.hypot(dy) < DRAG_THRESHOLD {
                return;
            }
            gesture.mode = match &gesture.start {
                Some(start) => Mode::Paint(!selection.picked.contains(start)),
                None if gesture.additive => Mode::Marquee(selection.picked.clone()),
                None => Mode::Marquee(HashSet::new()),
            };
            if let Some(start) = gesture.start.clone() {
                selection.anchor = Some(start);
            }
            self.close_card_menu(cx);
            Tooltips::dismiss(cx);
            motion::release_hovers(cx);
            self.start_autoscroll(window, cx);
        }
        let changed = self.home.selection.track(offset);
        if changed
            && matches!(
                self.home.selection.gesture.as_ref().map(|g| &g.mode),
                Some(Mode::Paint(_))
            )
        {
            sound::play(Sound::Tap);
        }
        cx.notify();
    }

    fn selection_up(&mut self, cx: &mut Context<Self>) {
        let selection = &mut self.home.selection;
        let Some(gesture) = selection.gesture.take() else {
            return;
        };
        selection.scroll_task = None;
        match gesture.mode {
            Mode::Pending => {
                if gesture.start.is_none() && !gesture.additive && selection.clear() {
                    cx.notify();
                }
            }
            _ => {
                selection.swallow_click = true;
                cx.notify();
            }
        }
    }

    fn start_autoscroll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.home.selection.scroll_task.is_some() {
            return;
        }
        self.home.selection.scroll_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(SCROLL_TICK).await;
                let alive = this
                    .update(cx, |shell, cx| shell.autoscroll(cx))
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        }));
    }

    fn autoscroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(gesture) = self.home.selection.gesture.as_ref() else {
            return false;
        };
        let scroll = &self.home.scroll;
        let viewport = scroll.bounds();
        let y = f32::from(gesture.pointer.y);
        let top = f32::from(viewport.top()) + EDGE_INSET;
        let bottom = f32::from(viewport.bottom()) - EDGE_INSET;
        let overshoot = if y < top {
            y - top
        } else if y > bottom {
            y - bottom
        } else {
            return true;
        };
        let speed =
            (overshoot.abs() * SCROLL_GAIN).clamp(SCROLL_MIN, SCROLL_MAX) * overshoot.signum();
        let offset = scroll.offset();
        let max = f32::from(scroll.max_offset().height);
        let next = (f32::from(offset.y) - speed).clamp(-max, 0.0);
        if (next - f32::from(offset.y)).abs() < 0.01 {
            return true;
        }
        scroll.set_offset(Point::new(offset.x, px(next)));
        let offset = scroll.offset();
        self.home.selection.track(offset);
        cx.notify();
        true
    }

    pub(crate) fn sync_pick_motion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selection = &self.home.selection;
        let mut moving = motion::sync(
            Key::of(SELECTING),
            !selection.is_empty(),
            MARK_FADE,
            motion::EASE_OUT,
            cx,
        );
        for uuid in &selection.order {
            let on = selection.picked.contains(uuid);
            moving |= motion::sync(Key::new(PICK, uuid), on, PICK_FADE, motion::EASE_OUT, cx);
            moving |= motion::sync(
                Key::new(PICK_POP, uuid),
                on,
                PICK_POP_TIME,
                motion::EASE_SPRING,
                cx,
            );
        }
        if moving {
            window.request_animation_frame();
        }
    }

    pub(crate) fn render_drag_overlay(&self, cx: &App) -> Option<gpui::Div> {
        let p = cx.palette();
        let scroll = &self.home.scroll;
        let viewport = scroll.bounds();
        let rect = self.home.selection.marquee(scroll.offset())?;
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .left(rect.origin.x - viewport.origin.x)
                        .top(rect.origin.y - viewport.origin.y)
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(Hsla { a: 0.8, ..p.blue })
                        .bg(Hsla { a: 0.12, ..p.blue }),
                ),
        )
    }

    pub(crate) fn forget_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.home.selection.is_empty() {
            return false;
        }
        self.run_bulk(BulkAction::Forget, window, cx);
        true
    }

    pub(crate) fn selection_listeners(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.home.selection.gesture.as_ref()?;
        let entity = cx.entity().downgrade();
        Some(
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let moving = entity.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase != DispatchPhase::Capture {
                            return;
                        }
                        let pressed = event.pressed_button == Some(MouseButton::Left);
                        moving
                            .update(cx, |shell, cx| {
                                shell.selection_move(event.position, pressed, window, cx)
                            })
                            .ok();
                    });
                    let releasing = entity.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                            return;
                        }
                        releasing
                            .update(cx, |shell, cx| shell.selection_up(cx))
                            .ok();
                    });
                },
            )
            .absolute()
            .size_0()
            .into_any_element(),
        )
    }

    pub(crate) fn selection_click(
        &mut self,
        uuid: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let selection = &mut self.home.selection;
        if std::mem::take(&mut selection.swallow_click) || selection.dragging() {
            return true;
        }
        if modifiers.shift && !selection.is_empty() {
            selection.extend_to(uuid);
        } else if modifiers.control || modifiers.platform || !selection.is_empty() {
            selection.toggle(uuid);
        } else {
            return false;
        }
        sound::play(Sound::Tap);
        cx.notify();
        true
    }

    pub(crate) fn toggle_picked(&mut self, uuid: &str, cx: &mut Context<Self>) {
        self.home.selection.toggle(uuid);
        sound::play(Sound::Tap);
        cx.notify();
    }

    pub(crate) fn select_all(&mut self, cx: &mut Context<Self>) {
        let selection = &mut self.home.selection;
        selection.picked = selection.order.iter().cloned().collect();
        cx.notify();
    }

    pub(crate) fn clear_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let cleared = self.home.selection.clear();
        if cleared {
            if matches!(self.popup, Some(Popup::Selection { .. })) {
                self.set_popup(None);
            }
            cx.notify();
        }
        cleared
    }

    fn picked_accounts(&self, cx: &App) -> Vec<Account> {
        let store = self.store.read(cx);
        let selection = &self.home.selection;
        let mut ordered: Vec<Account> = selection
            .order
            .iter()
            .filter(|uuid| selection.picked.contains(*uuid))
            .filter_map(|uuid| store.account(uuid).cloned())
            .collect();
        let listed: HashSet<&str> = selection.order.iter().map(String::as_str).collect();
        ordered.extend(
            store
                .accounts
                .iter()
                .filter(|account| {
                    selection.picked.contains(&account.uuid)
                        && !listed.contains(account.uuid.as_str())
                })
                .cloned(),
        );
        ordered
    }

    pub(crate) fn prune_selection(&mut self, cx: &App) {
        let store = self.store.read(cx);
        if !store.accounts_loaded {
            return;
        }
        let known: HashSet<&str> = store.accounts.iter().map(|a| a.uuid.as_str()).collect();
        self.home
            .selection
            .picked
            .retain(|uuid| known.contains(uuid.as_str()));
    }

    fn run_bulk(&mut self, action: BulkAction, window: &mut Window, cx: &mut Context<Self>) {
        let accounts = self.picked_accounts(cx);
        if accounts.is_empty() {
            return;
        }
        match action {
            BulkAction::Launch => self.store.update(cx, |store, cx| {
                let mut started = 0;
                for account in &accounts {
                    if !account.launchable || account.running || account.launching {
                        continue;
                    }
                    if !store.begin_launch(&account.uuid, cx) {
                        continue;
                    }
                    let client = (!account.client.is_empty()).then(|| account.client.clone());
                    let future = store.backend.launch(account.uuid.clone(), client);
                    store.act(cx, future, "");
                    started += 1;
                }
                if started == 0 {
                    store.toast(
                        "Нечего запускать - аккаунты уже запущены или без токена",
                        true,
                        cx,
                    );
                } else {
                    store.toast(
                        format!("Запускаю {started} {}…", accounts_word(started)),
                        false,
                        cx,
                    );
                }
            }),
            BulkAction::Pin(pinned) => self.store.update(cx, |store, cx| {
                for account in accounts.iter().filter(|account| account.pinned != pinned) {
                    let future = store.backend.set_pinned(account.uuid.clone(), pinned);
                    store.act(cx, future, "");
                }
            }),
            BulkAction::AddTo(group) => {
                let name = self
                    .store
                    .read(cx)
                    .group(&group)
                    .map(|group| group.name.clone())
                    .unwrap_or_default();
                self.store.update(cx, |store, cx| {
                    for account in &accounts {
                        let future = store
                            .backend
                            .add_group_member(group.clone(), account.uuid.clone());
                        store.act(cx, future, "");
                    }
                    store.toast(
                        format!(
                            "Добавлено в «{name}»: {} {}",
                            accounts.len(),
                            accounts_word(accounts.len())
                        ),
                        false,
                        cx,
                    );
                });
            }
            BulkAction::NewGroup => {
                let members = accounts.into_iter().map(|account| account.uuid).collect();
                self.create_group_flow(members, window, cx);
            }
            BulkAction::RemoveFrom(group) => {
                self.store.update(cx, |store, cx| {
                    for account in &accounts {
                        let future = store
                            .backend
                            .remove_group_member(group.clone(), account.uuid.clone());
                        store.act(cx, future, "");
                    }
                });
                self.clear_selection(cx);
            }
            BulkAction::Forget => {
                let count = accounts.len();
                let uuids: Vec<String> = accounts.into_iter().map(|account| account.uuid).collect();
                self.confirm(
                    format!("Удалить {count} {}?", accounts_word(count)),
                    "Выбранные аккаунты будут убраны из списка.",
                    "Да, удалить",
                    move |this, _, cx| {
                        this.store.update(cx, |store, cx| {
                            for uuid in &uuids {
                                let future = store.backend.forget(uuid.clone());
                                store.act(cx, future, "");
                            }
                            store.toast(
                                format!("Удалено: {count} {}", accounts_word(count)),
                                false,
                                cx,
                            );
                        });
                        this.clear_selection(cx);
                    },
                    window,
                    cx,
                );
            }
            BulkAction::Clear => {
                self.clear_selection(cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn render_selection_bar(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.home.selection.is_empty() {
            return None;
        }
        let p = cx.palette();
        let store = self.store.read(cx);
        let selection = &self.home.selection;
        let (count, all_pinned) = store
            .accounts
            .iter()
            .filter(|account| selection.picked.contains(&account.uuid))
            .fold((0, true), |(count, all), account| {
                (count + 1, all && account.pinned)
            });
        let group = self.active_group(store).map(|group| group.id.clone());
        let everything = selection
            .order
            .iter()
            .all(|uuid| selection.picked.contains(uuid));
        let needed = LABELS_MIN_WIDTH
            + if group.is_some() {
                REMOVE_LABEL_WIDTH
            } else {
                0.0
            }
            + if everything { 0.0 } else { ALL_LABEL_WIDTH };
        let labels = f32::from(window.viewport_size().width) >= needed;

        let action = |id: &'static str,
                      glyph: Icon,
                      label: &'static str,
                      danger: bool,
                      cx: &mut Context<Self>| {
            let key = Key::of(id);
            let hover = motion::value(key, cx);
            let tone = if danger { p.red } else { p.text };
            let element = div()
                .id(id)
                .flex()
                .flex_none()
                .items_center()
                .gap(px(7.0))
                .h(px(32.0))
                .px(px(if labels { 11.0 } else { 9.0 }))
                .rounded(px(8.0))
                .bg(mix(rgba(0x000000, 0.0), p.wash, hover))
                .text_size(rem(0.8))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(mix(if danger { p.red } else { p.muted }, tone, hover))
                .whitespace_nowrap()
                .cursor_pointer()
                .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
                .child(icon(
                    glyph,
                    px(15.0),
                    mix(if danger { p.red } else { p.muted }, tone, hover),
                ))
                .when(labels, |element| element.child(label));
            if labels {
                element
            } else {
                with_tooltip(element, key, label.into())
            }
        };

        let launch = action("bulk-launch", Icon::Play, "Запустить", false, cx).on_click(
            cx.listener(|this, _, window, cx| {
                sound::play(Sound::Confirm);
                this.run_bulk(BulkAction::Launch, window, cx);
            }),
        );
        let pin = action(
            "bulk-pin",
            if all_pinned {
                Icon::StarSolid
            } else {
                Icon::Star
            },
            if all_pinned {
                "Открепить"
            } else {
                "Закрепить"
            },
            false,
            cx,
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            sound::play(Sound::Tap);
            this.run_bulk(BulkAction::Pin(!all_pinned), window, cx);
        }));
        let groups = action("bulk-group", Icon::Users, "В группу", false, cx).on_click(
            cx.listener(|this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                sound::play(Sound::Tap);
                let position = event.position();
                this.toggle_popup(
                    Popup::Selection {
                        x: f32::from(position.x),
                        y: f32::from(position.y),
                        full: false,
                    },
                    cx,
                );
            }),
        );
        let remove = group.map(|id| {
            action(
                "bulk-remove",
                Icon::CloseCircle,
                "Убрать из группы",
                false,
                cx,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                sound::play(Sound::Tap);
                this.run_bulk(BulkAction::RemoveFrom(id.clone()), window, cx);
            }))
        });
        let forget = action("bulk-forget", Icon::Trash, "Удалить", true, cx).on_click(cx.listener(
            |this, _, window, cx| {
                sound::play(Sound::Tap);
                this.run_bulk(BulkAction::Forget, window, cx);
            },
        ));
        let close_key = Key::of("bulk-clear");
        let close_hover = motion::value(close_key, cx);
        let close = with_tooltip(
            div()
                .id("bulk-clear")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(32.0))
                .rounded(px(8.0))
                .bg(mix(rgba(0x000000, 0.0), p.wash, close_hover))
                .cursor_pointer()
                .on_hover(move |hovered, window, cx| motion::hover(close_key, *hovered, window, cx))
                .on_click(cx.listener(|this, _, window, cx| {
                    sound::play(Sound::Tap);
                    this.run_bulk(BulkAction::Clear, window, cx);
                }))
                .child(icon(
                    Icon::Close,
                    px(14.0),
                    mix(p.muted, p.text, close_hover),
                )),
            close_key,
            "Снять выделение (Esc)".into(),
        );
        let uncheck_key = Key::of("bulk-uncheck");
        let uncheck_hover = motion::value(uncheck_key, cx);
        let divider = || {
            div()
                .flex_none()
                .w(px(1.0))
                .h(px(20.0))
                .mx(px(4.0))
                .bg(p.line)
        };

        let bar = div()
            .id("selection-bar")
            .flex()
            .items_center()
            .gap(px(2.0))
            .h(px(46.0))
            .pl(px(14.0))
            .pr(px(7.0))
            .rounded(px(13.0))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.surface)
            .shadow(shadow(0.0, 10.0, 30.0, 0.0, rgba(0x000000, p.popup_shadow)))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(9.0))
                    .mr(px(6.0))
                    .child(with_tooltip(
                        div()
                            .id("bulk-uncheck")
                            .flex_none()
                            .cursor_pointer()
                            .opacity(1.0 - 0.25 * uncheck_hover)
                            .on_hover(move |hovered, window, cx| {
                                motion::hover(uncheck_key, *hovered, window, cx)
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                sound::play(Sound::Tap);
                                this.run_bulk(BulkAction::Clear, window, cx);
                            }))
                            .child(Checkbox::new(true)),
                        uncheck_key,
                        "Снять выделение".into(),
                    ))
                    .child(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .text_size(rem(0.84))
                            .font_weight(FontWeight::BOLD)
                            .child(SharedString::from(if labels {
                                format!("{} {count}", picked_word(count))
                            } else {
                                count.to_string()
                            })),
                    ),
            )
            .when(!everything, |bar| {
                bar.child(
                    action("bulk-all", Icon::Check, "Выбрать все", false, cx).on_click(
                        cx.listener(|this, _, _, cx| {
                            sound::play(Sound::Tap);
                            this.select_all(cx);
                        }),
                    ),
                )
            })
            .child(divider())
            .child(launch)
            .child(pin)
            .child(groups)
            .children(remove)
            .child(forget)
            .child(divider())
            .child(close);

        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(16.0))
                .flex()
                .justify_center()
                .child(bar.occlude().with_animation(
                    "selection-bar-in",
                    Animation::new(BAR_IN).with_easing(motion::eased(motion::EASE)),
                    |bar, t| bar.opacity(t).mt(px(12.0 * (1.0 - t))),
                ))
                .into_any_element(),
        )
    }

    pub(crate) fn render_selection_menu(
        &mut self,
        x: f32,
        y: f32,
        full: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let p = cx.palette();
        let accounts = self.picked_accounts(cx);
        if accounts.is_empty() {
            return None;
        }
        let count = accounts.len();
        let store = self.store.read(cx);
        let active = self.active_group(store).cloned();
        let groups: Vec<(String, String)> = store
            .groups
            .iter()
            .filter(|group| active.as_ref().is_none_or(|active| active.id != group.id))
            .map(|group| (group.id.clone(), group.name.clone()))
            .collect();
        let all_pinned = accounts.iter().all(|account| account.pinned);

        let mut items: Vec<(SharedString, MenuItem, BulkAction)> = Vec::new();
        let mut separators = Vec::new();
        if full {
            items.push((
                format!("Запустить {count} {}", accounts_word(count)).into(),
                MenuItem::Plain,
                BulkAction::Launch,
            ));
            items.push((
                if all_pinned {
                    "Открепить"
                } else {
                    "Закрепить"
                }
                .into(),
                MenuItem::Plain,
                BulkAction::Pin(!all_pinned),
            ));
            separators.push(items.len());
        }
        for (id, name) in groups {
            items.push((
                format!("В «{name}»").into(),
                MenuItem::Plain,
                BulkAction::AddTo(id),
            ));
        }
        items.push((
            "Новая группа…".into(),
            MenuItem::Action,
            BulkAction::NewGroup,
        ));
        if full {
            separators.push(items.len());
            if let Some(group) = &active {
                items.push((
                    format!("Убрать из «{}»", group.name).into(),
                    MenuItem::Danger,
                    BulkAction::RemoveFrom(group.id.clone()),
                ));
            }
            items.push((
                format!("Удалить {count} {}", accounts_word(count)).into(),
                MenuItem::Danger,
                BulkAction::Forget,
            ));
        }

        let item_height = 13.6 * LINE_HEIGHT + 18.0;
        let height =
            items.len() as f32 * (item_height + 2.0) + separators.len() as f32 * 9.0 + 14.0;
        let viewport = window.viewport_size();
        let max_height = f32::from(viewport.height) - 16.0;
        let shown = height.min(max_height);
        let left = if full { x } else { x - MENU_WIDTH / 2.0 }
            .min(f32::from(viewport.width) - MENU_WIDTH - 8.0)
            .max(8.0);
        let top = if full { y } else { y - shown - 26.0 }
            .min(f32::from(viewport.height) - shown - 8.0)
            .max(8.0);

        let mut menu = menu_surface(p.surface, 11.0, 6.0, cx)
            .gap(px(2.0))
            .w(px(MENU_WIDTH))
            .max_h(px(max_height))
            .text_size(rem(0.85));
        for (index, (label, kind, action)) in items.into_iter().enumerate() {
            if separators.contains(&index) {
                menu = menu.child(menu_separator(&p, 6.0, 4.0));
            }
            menu = menu.child(
                menu_item(
                    ElementId::Name(format!("bulk-item-{index}").into()),
                    kind,
                    &p,
                )
                .px(px(12.0))
                .py(px(9.0))
                .rounded(px(7.0))
                .whitespace_nowrap()
                .on_click(cx.listener(move |this, _, window, cx| {
                    sound::play(Sound::Tap);
                    this.popup = None;
                    this.run_bulk(action.clone(), window, cx);
                    cx.notify();
                }))
                .child(label),
            );
        }
        Some(
            deferred(
                div().absolute().left(px(left)).top(px(top)).child(drop_in(
                    "selection-menu-in",
                    menu.id("selection-menu")
                        .overflow_y_scroll()
                        .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                            this.dismiss_popup(Popup::Selection { x, y, full }, cx)
                        })),
                    None,
                )),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    fn card(selection: &Selection, uuid: &str, x: f32, y: f32) {
        selection.cards.borrow_mut().insert(
            uuid.to_owned(),
            Bounds::new(Point::new(px(x), px(y)), size(px(100.0), px(60.0))),
        );
    }

    fn grid() -> Selection {
        let selection = Selection::default();
        card(&selection, "a", 0.0, 0.0);
        card(&selection, "b", 110.0, 0.0);
        card(&selection, "c", 0.0, 70.0);
        card(&selection, "d", 110.0, 70.0);
        selection
    }

    fn drag(selection: &mut Selection, from: (f32, f32), mode: Mode) {
        let origin = Point::new(px(from.0), px(from.1));
        selection.gesture = Some(Gesture {
            origin,
            pointer: origin,
            last: origin,
            start: selection.card_at(origin),
            additive: false,
            mode,
        });
    }

    fn move_to(selection: &mut Selection, to: (f32, f32), offset: Point<Pixels>) {
        selection.gesture.as_mut().unwrap().pointer = Point::new(px(to.0), px(to.1));
        selection.track(offset);
    }

    fn picked(selection: &Selection) -> Vec<&str> {
        let mut picked: Vec<&str> = selection.picked.iter().map(String::as_str).collect();
        picked.sort_unstable();
        picked
    }

    #[test]
    fn marquee_picks_touched_cards() {
        let mut selection = grid();
        drag(&mut selection, (105.0, -5.0), Mode::Marquee(HashSet::new()));
        move_to(&mut selection, (150.0, 30.0), Point::default());
        assert_eq!(picked(&selection), ["b"]);
        move_to(&mut selection, (50.0, 90.0), Point::default());
        assert_eq!(picked(&selection), ["a", "c"]);
        move_to(&mut selection, (160.0, 90.0), Point::default());
        assert_eq!(picked(&selection), ["b", "d"]);
        move_to(&mut selection, (106.0, 20.0), Point::default());
        assert!(selection.picked.is_empty());
    }

    #[test]
    fn marquee_keeps_base_and_follows_scroll() {
        let mut selection = grid();
        selection.picked.insert("a".to_owned());
        let base = selection.picked.clone();
        drag(&mut selection, (105.0, 65.0), Mode::Marquee(base));
        move_to(
            &mut selection,
            (150.0, 20.0),
            Point::new(px(0.0), px(-60.0)),
        );
        assert_eq!(picked(&selection), ["a", "d"]);
    }

    #[test]
    fn paint_sweeps_cards_between_moves() {
        let mut selection = grid();
        drag(&mut selection, (50.0, 30.0), Mode::Paint(true));
        selection.picked.insert("a".to_owned());
        move_to(&mut selection, (160.0, 100.0), Point::default());
        assert!(selection.contains("d"));
        drag(&mut selection, (160.0, 100.0), Mode::Paint(false));
        move_to(&mut selection, (50.0, 100.0), Point::default());
        assert!(!selection.contains("d"));
        assert!(selection.contains("a"));
    }

    #[test]
    fn shift_extends_in_visible_order() {
        let mut selection = grid();
        selection.order = ["a", "b", "c", "d"].map(str::to_owned).to_vec();
        selection.toggle("b");
        selection.extend_to("d");
        assert_eq!(picked(&selection), ["b", "c", "d"]);
        assert!(selection.clear());
        assert!(!selection.clear());
    }

    #[test]
    fn plural_forms() {
        assert_eq!(accounts_word(1), "аккаунт");
        assert_eq!(accounts_word(3), "аккаунта");
        assert_eq!(accounts_word(5), "аккаунтов");
        assert_eq!(accounts_word(11), "аккаунтов");
        assert_eq!(accounts_word(22), "аккаунта");
    }
}
