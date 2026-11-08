#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ambient;
mod assets;
mod background;
mod format;
#[cfg(feature = "mock")]
mod mock;
mod motion;
mod platform;
mod skin;
mod sound;
mod state;
mod theme;
mod ui;
mod views;

use std::sync::Arc;

use ac_api::Backend;
use futures::StreamExt;
use futures::channel::mpsc::unbounded;
use gpui::{
    App, AppContext, Application, Focusable, SharedString, TitlebarOptions, WindowBounds,
    WindowOptions, px, size,
};

use crate::assets::Assets;
use crate::platform::instance;
use crate::views::shell::Shell;

struct Options {
    mock: bool,
    updated: bool,
}

impl Options {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect();
        Self {
            mock: cfg!(feature = "mock") && args.iter().any(|arg| arg == "--mock"),
            updated: args.iter().any(|arg| arg == "--updated"),
        }
    }
}

fn backend(options: &Options) -> anyhow::Result<Arc<dyn Backend>> {
    #[cfg(feature = "mock")]
    if options.mock {
        return Ok(Arc::new(mock::MockBackend::new()));
    }
    let _ = options;
    Ok(Arc::new(ac_core::Core::start(
        ac_platform::Paths::resolve(),
    )?))
}

fn main() -> anyhow::Result<()> {
    let options = Options::parse();
    let port = instance::port();
    let listener = if options.mock {
        None
    } else {
        match instance::acquire(port, options.updated) {
            Some(listener) => Some(listener),
            None => {
                instance::request_focus(port);
                return Ok(());
            }
        }
    };
    let backend = backend(&options)?;
    if !options.mock {
        ac_platform::install_desktop_entry(assets::APP_ICON_PNG).ok();
    }

    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            let font = assets::register_fonts(cx);
            ui::bind_input_keys(cx);
            theme::install(ac_api::Theme::Dark, cx);
            let store = state::new_store(backend.clone(), cx);
            let bounds = platform::window::initial_bounds(cx);
            let window = cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::new_static("AccountChanger")),
                        appears_transparent: true,
                        traffic_light_position: None,
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(480.0), px(420.0))),
                    app_id: Some("AccountChanger".to_owned()),
                    ..WindowOptions::default()
                },
                |window, cx| {
                    let shell = cx.new(|cx| Shell::new(store.clone(), font.clone(), window, cx));
                    window.focus(&shell.focus_handle(cx));
                    let closing = shell.downgrade();
                    window.on_window_should_close(cx, move |window, cx| {
                        if let Some(shell) = closing.upgrade() {
                            shell.update(cx, |shell, cx| shell.close_to_background(window, cx));
                        }
                        false
                    });
                    shell
                },
            );
            let Ok(window) = window else {
                cx.quit();
                return;
            };

            let (focus_sender, mut focus_requests) = unbounded::<()>();
            if let Some(listener) = listener {
                instance::serve(listener, focus_sender.clone());
            }
            cx.spawn(async move |cx| {
                while focus_requests.next().await.is_some() {
                    let revealed = window.update(cx, |shell, window, cx| shell.reveal(window, cx));
                    if revealed.is_err() {
                        break;
                    }
                }
            })
            .detach();

            #[cfg(windows)]
            {
                use crate::platform::tray::{Tray, TrayCommand};
                let (commands, mut tray_events) = unbounded::<TrayCommand>();
                let menu_commands = commands.clone();
                let tray = Tray::install(commands);
                cx.spawn(async move |cx| {
                    let _tray = tray;
                    while let Some(command) = tray_events.next().await {
                        match command {
                            TrayCommand::Open => {
                                if window
                                    .update(cx, |shell, window, cx| shell.reveal(window, cx))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            TrayCommand::Quit => {
                                cx.update(|cx| cx.quit()).ok();
                                break;
                            }
                            TrayCommand::Menu { x, y } => {
                                let commands = menu_commands.clone();
                                cx.update(|cx| {
                                    platform::tray_menu::TrayMenu::open(x, y, commands, cx)
                                })
                                .ok();
                            }
                        }
                    }
                })
                .detach();
            }

            cx.activate(true);
        });
    Ok(())
}
