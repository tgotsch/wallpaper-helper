use std::sync::{Arc, Mutex};

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::{Config, WindowBuilder, WindowCloseBehaviour};
use tokio::sync::mpsc::UnboundedSender;

use crate::tray::TrayAction;

mod backend;
mod logging;
mod tray;
mod ui;
mod wallpaper_manager;
//mod wallpaper_source;

/// Single-instance enforcement (replaces the GTK Application ID): if another
/// instance already owns the local socket, ask it to show its window and exit.
/// Otherwise listen and forward "show window" requests into the action channel.
fn start_single_instance_listener_or_forward(tx: UnboundedSender<TrayAction>) {
    use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Stream};
    use std::io::Write;

    let name = match "com.wallpaperhelper.app.sock".to_ns_name::<GenericNamespaced>() {
        Ok(name) => name,
        Err(e) => {
            log::warn!("Single-instance socket name invalid: {}", e);
            return;
        }
    };

    if let Ok(mut conn) = Stream::connect(name.clone()) {
        let _ = conn.write_all(b"SHOW");
        log::info!("Another instance is already running; asked it to show its window.");
        std::process::exit(0);
    }

    match ListenerOptions::new().name(name).create_sync() {
        Ok(listener) => {
            std::thread::spawn(move || {
                for _conn in listener.incoming().flatten() {
                    let _ = tx.send(TrayAction::ShowWindow);
                }
            });
        }
        Err(e) => {
            log::warn!("Could not bind single-instance socket: {}", e);
        }
    }
}

fn main() {
    logging::init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.json".to_string());

    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel::<TrayAction>();
    start_single_instance_listener_or_forward(action_tx.clone());

    let init = ui::AppInit {
        config_path,
        action_tx,
        action_rx: Arc::new(Mutex::new(Some(action_rx))),
    };

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(
                    WindowBuilder::new()
                        .with_title("Wallpaper Helper")
                        .with_inner_size(LogicalSize::new(1000.0, 1000.0)),
                )
                .with_menu(None)
                .with_close_behaviour(WindowCloseBehaviour::WindowHides)
                .with_disable_context_menu(!cfg!(debug_assertions)),
        )
        .with_context(init)
        .launch(ui::app::App);
}
