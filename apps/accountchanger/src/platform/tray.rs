use futures::channel::mpsc::UnboundedSender;
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::assets::APP_ICON_PNG;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrayCommand {
    Open,
    Quit,
    Menu { x: f32, y: f32 },
}

pub struct Tray {
    _icon: TrayIcon,
}

fn icon() -> Option<Icon> {
    let image = image::load_from_memory(APP_ICON_PNG).ok()?.into_rgba8();
    let image = image::imageops::resize(&image, 32, 32, image::imageops::FilterType::Lanczos3);
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).ok()
}

fn system_scale() -> f32 {
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    unsafe { GetDpiForSystem() }.max(96) as f32 / 96.0
}

impl Tray {
    pub fn install(commands: UnboundedSender<TrayCommand>) -> Option<Self> {
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            let command = match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
                | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => Some(TrayCommand::Open),
                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    position,
                    ..
                } => {
                    let scale = system_scale();
                    Some(TrayCommand::Menu {
                        x: position.x as f32 / scale,
                        y: position.y as f32 / scale,
                    })
                }
                _ => None,
            };
            if let Some(command) = command {
                commands.unbounded_send(command).ok();
            }
        }));
        let icon = TrayIconBuilder::new()
            .with_menu_on_left_click(false)
            .with_tooltip("AccountChanger")
            .with_icon(icon()?)
            .build()
            .ok()?;
        Some(Self { _icon: icon })
    }
}
