use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, Bounds, Context, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Rgba, StatefulInteractiveElement, Styled,
    Task, Window, canvas, div, prelude::FluentBuilder, px,
};

use crate::assets::{Icon, icon};
use crate::cabinet_login::{self, LoginView, Prefill};
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, mix, rem, rgba, shadow};
use crate::ui::{Spinner, Tooltips};
use crate::views::shell::Shell;

const APPEAR: Duration = Duration::from_millis(220);
const POLL: Duration = Duration::from_millis(250);
const CARD_WIDTH: f32 = 400.0;
const CARD_HEIGHT: f32 = 470.0;

pub struct CabinetLogin {
    uuid: String,
    opened: Instant,
    prefill: Option<Option<Prefill>>,
    slot: Rc<Cell<Option<Bounds<Pixels>>>>,
    view: Option<LoginView>,
    _poll: Task<()>,
}

fn rgb8(color: Hsla) -> (u8, u8, u8) {
    let rgba: Rgba = color.into();
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    (channel(rgba.r), channel(rgba.g), channel(rgba.b))
}

impl Shell {
    pub(crate) fn open_cabinet_login(&mut self, uuid: String, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let username = store
            .account(&uuid)
            .map(|account| account.name.clone())
            .unwrap_or_default();
        let password = store.backend.account_password(uuid.clone());
        let poll = cx.spawn(async move |this, cx| {
            let password = password.await.ok().flatten().unwrap_or_default();
            let prefill = (!username.is_empty()).then_some(Prefill { username, password });
            this.update(cx, |shell, cx| {
                if let Some(login) = shell.cabinet_login.as_mut() {
                    login.prefill = Some(prefill);
                }
                cx.notify();
            })
            .ok();
            loop {
                cx.background_executor().timer(POLL).await;
                let open = this
                    .update(cx, |shell, cx| {
                        let open = shell.cabinet_login.is_some();
                        if open {
                            cx.notify();
                        }
                        open
                    })
                    .unwrap_or(false);
                if !open {
                    break;
                }
            }
        });
        Tooltips::dismiss(cx);
        motion::release_hovers(cx);
        self.cabinet_login = Some(CabinetLogin {
            uuid,
            opened: Instant::now(),
            prefill: None,
            slot: Rc::new(Cell::new(None)),
            view: None,
            _poll: poll,
        });
        cx.notify();
    }

    pub(crate) fn close_cabinet_login(&mut self, cx: &mut Context<Self>) -> bool {
        if self.cabinet_login.take().is_none() {
            return false;
        }
        cx.notify();
        true
    }

    pub(crate) fn drive_cabinet_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(login) = self.cabinet_login.as_mut() else {
            return;
        };
        if login.view.is_none() {
            let created = match (
                login.opened.elapsed() >= APPEAR,
                login.prefill.as_ref(),
                login.slot.get(),
            ) {
                (true, Some(prefill), Some(bounds)) => Some(cabinet_login::create(
                    window,
                    bounds,
                    prefill.as_ref(),
                    rgb8(cx.palette().surface),
                )),
                _ => None,
            };
            match created {
                Some(Ok(view)) => login.view = Some(view),
                Some(Err(message)) => {
                    self.cabinet_login = None;
                    self.toast(message, true, cx);
                    return;
                }
                None => {
                    window.request_animation_frame();
                    return;
                }
            }
        }
        let Some(login) = self.cabinet_login.as_ref() else {
            return;
        };
        let Some(view) = login.view.as_ref() else {
            return;
        };
        if let Some(bounds) = login.slot.get() {
            view.place(bounds, window.scale_factor());
        }
        let Some(result) = view.session() else {
            return;
        };
        let uuid = login.uuid.clone();
        self.cabinet_login = None;
        match result {
            Ok(signed) => self.store.update(cx, |store, cx| {
                let own = store
                    .account(&uuid)
                    .is_some_and(|account| account.name.eq_ignore_ascii_case(&signed.login));
                if own && !signed.password.is_empty() {
                    store.save_account_password(uuid.clone(), signed.password, cx);
                }
                store.save_cabinet_session(uuid, signed.session, cx);
                store.toast("Кабинет подключён", false, cx);
            }),
            Err(message) => self.toast(message, true, cx),
        }
        cx.notify();
    }

    pub(crate) fn render_cabinet_login(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let login = self.cabinet_login.as_ref()?;
        let p = cx.palette();
        let appear = (login.opened.elapsed().as_secs_f32() / APPEAR.as_secs_f32()).min(1.0);
        let slot = login.slot.clone();
        let loading = login.view.is_none();
        let close_key = Key::of("cabinet-login-close");
        let close_hover = motion::value(close_key, cx);

        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .pl(px(20.0))
            .pr(px(12.0))
            .pt(px(14.0))
            .pb(px(6.0))
            .child(
                div()
                    .text_size(rem(0.95))
                    .font_weight(FontWeight::BOLD)
                    .child("Вход в кабинет Cristalix"),
            )
            .child(
                div()
                    .id("cabinet-login-close")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(30.0))
                    .rounded(px(8.0))
                    .bg(mix(rgba(0x000000, 0.0), p.wash, close_hover))
                    .cursor_pointer()
                    .on_hover(move |hovered, window, cx| {
                        motion::hover(close_key, *hovered, window, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        sound::play(Sound::Tap);
                        this.close_cabinet_login(cx);
                    }))
                    .child(icon(
                        Icon::Close,
                        px(14.0),
                        mix(p.muted, p.text, close_hover),
                    )),
            );

        let body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .mx(px(8.0))
            .mb(px(8.0))
            .child(
                canvas(move |bounds, _, _| slot.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
            .when(loading, |body| {
                body.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Spinner::new("cabinet-login-spin", 22.0)),
                )
            });

        let card = div()
            .id("cabinet-login-card")
            .flex()
            .flex_col()
            .w(px(CARD_WIDTH))
            .h(px(CARD_HEIGHT))
            .rounded(px(14.0))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.surface)
            .shadow(shadow(0.0, 18.0, 50.0, 0.0, rgba(0x000000, p.popup_shadow)))
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(header)
            .child(body)
            .with_animation(
                "cabinet-login-in",
                Animation::new(APPEAR).with_easing(motion::eased(motion::EASE_OUT)),
                |card, t| card.opacity(t).mt(px(18.0 * (1.0 - t))),
            );

        Some(
            div()
                .id("cabinet-login-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(0x000000, 0.45 * appear))
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.close_cabinet_login(cx);
                    }),
                )
                .child(card)
                .into_any_element(),
        )
    }
}
