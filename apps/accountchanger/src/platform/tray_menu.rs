use futures::channel::mpsc::UnboundedSender;
use gpui::{
    App, AppContext, Bounds, Context, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Window, WindowBounds, WindowKind,
    WindowOptions, div, point, px, size,
};

use crate::platform::tray::TrayCommand;
use crate::theme::{rgb, rgba, shadow};

const WIDTH: f32 = 188.0;
const PAD: f32 = 8.0;
const ITEM: f32 = 34.0;
const GAP: f32 = 2.0;
const ITEMS: [(&str, TrayCommand); 2] = [
    ("Открыть", TrayCommand::Open),
    ("Закрыть", TrayCommand::Quit),
];

pub struct TrayMenu {
    commands: UnboundedSender<TrayCommand>,
    _activation: Subscription,
}

impl TrayMenu {
    fn height() -> f32 {
        PAD * 2.0 + ITEMS.len() as f32 * ITEM + (ITEMS.len() as f32 - 1.0) * GAP
    }

    pub fn open(x: f32, y: f32, commands: UnboundedSender<TrayCommand>, cx: &mut App) {
        let screen = cx
            .primary_display()
            .map(|display| display.bounds().size)
            .unwrap_or(size(px(1920.0), px(1080.0)));
        let height = Self::height();
        let left = x.min(f32::from(screen.width) - WIDTH - 4.0).max(0.0);
        let top = y.min(f32::from(screen.height) - height - 4.0).max(0.0);
        cx.open_window(
            WindowOptions {
                titlebar: None,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                focus: true,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(left), px(top)),
                    size(px(WIDTH), px(height)),
                ))),
                ..WindowOptions::default()
            },
            |window, cx| {
                cx.new(|cx| TrayMenu {
                    commands,
                    _activation: cx.observe_window_activation(window, |_, window, _| {
                        if !window.is_window_active() {
                            window.remove_window();
                        }
                    }),
                })
            },
        )
        .ok();
    }
}

impl Render for TrayMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .p(px(PAD))
            .bg(rgb(0x1c1c1c))
            .shadow(shadow(0.0, 4.0, 14.0, 0.0, rgba(0x000000, 0.4)))
            .font_family("Segoe UI")
            .text_size(px(14.0))
            .text_color(rgb(0xf0f0f0))
            .children(ITEMS.into_iter().map(|(label, command)| {
                div()
                    .id(label)
                    .flex()
                    .items_center()
                    .h(px(ITEM))
                    .pl(px(12.0))
                    .rounded(px(7.0))
                    .cursor_pointer()
                    .hover(|item| item.bg(rgb(0x2a2a2a)))
                    .on_click(cx.listener(move |this, _, window, _| {
                        this.commands.unbounded_send(command).ok();
                        window.remove_window();
                    }))
                    .child(label)
            }))
    }
}
