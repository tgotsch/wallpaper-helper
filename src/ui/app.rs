use std::collections::HashMap;
use std::rc::Rc;

use dioxus::desktop::{use_asset_handler, use_window, use_wry_event_handler};
use dioxus::prelude::*;
use percent_encoding::percent_decode_str;

use crate::tray::{AppTray, TrayAction};
use crate::ui::{
    collections_view::CollectionsView, dialogs::{ConfirmModal, NameModal},
    monitors_view::MonitorsView, profiles_view::ProfilesView, sidebar::Sidebar, sorted_profiles,
    AppCtx, AppInit, CollectionStep, SlideshowState, View,
};
use crate::wallpaper_manager::WallpaperManager;

#[component]
pub fn App() -> Element {
    let init = use_context::<AppInit>();

    let manager = use_signal(|| {
        let mut m = WallpaperManager::new();
        m.load_config(&init.config_path);
        m
    });

    let selected_profile = use_signal(|| {
        let mgr = manager.peek();
        let names = sorted_profiles(&mgr);
        if names.iter().any(|n| n == "default") {
            Some("default".to_string())
        } else {
            names.first().cloned()
        }
    });

    let selected_collection = use_signal(|| {
        let mgr = manager.peek();
        crate::ui::sorted_collections(&mgr).first().cloned()
    });

    let draft = use_signal(|| {
        let mgr = manager.peek();
        match &*selected_profile.peek() {
            Some(name) => crate::ui::draft_from_profile(&mgr, name),
            None => HashMap::new(),
        }
    });

    let view = use_signal(|| View::Profiles);
    let status = use_signal(String::new);
    let slideshow = use_signal(SlideshowState::default);
    let name_modal = use_signal(|| None);
    let confirm_modal = use_signal(|| None);
    let window_visible = use_signal(|| true);

    let tray = use_hook(|| Rc::new(AppTray::new(init.action_tx.clone())));

    let ctx = use_context_provider(|| AppCtx {
        manager,
        config_path: Rc::from(init.config_path.as_str()),
        view,
        selected_profile,
        selected_collection,
        draft,
        status,
        slideshow,
        name_modal,
        confirm_modal,
        window_visible,
        tray: tray.clone(),
    });

    // Keep selections valid as profiles/collections are created and deleted.
    {
        let ctx = ctx.clone();
        use_effect(move || {
            let mgr = manager.read();
            let profiles = sorted_profiles(&mgr);
            let collections = crate::ui::sorted_collections(&mgr);
            drop(mgr);

            let profile_ok = selected_profile
                .peek()
                .as_ref()
                .is_some_and(|p| profiles.contains(p));
            if !profile_ok {
                let fallback = if profiles.iter().any(|n| n == "default") {
                    Some("default".to_string())
                } else {
                    profiles.first().cloned()
                };
                ctx.select_profile(fallback);
            }

            let collection_ok = selected_collection
                .peek()
                .as_ref()
                .is_some_and(|c| collections.contains(c));
            if !collection_ok {
                ctx.selected_collection.clone().set(collections.first().cloned());
            }
        });
    }

    // --- Wallpaper previews: serve local files to the webview. A `?w=` query
    // returns a cached downscaled JPEG instead of the raw file (decoding
    // full-resolution wallpapers for small previews made the UI lag). ---
    use_asset_handler("wallpaper", move |request, responder| {
        let decoded = percent_decode_str(request.uri().path().trim_start_matches("/wallpaper/"))
            .decode_utf8_lossy()
            .to_string();
        let width: Option<u32> = request.uri().query().and_then(|q| {
            q.split('&')
                .find_map(|kv| kv.strip_prefix("w="))
                .and_then(|w| w.parse().ok())
        });
        std::thread::spawn(move || {
            use dioxus::desktop::wry::http::Response;
            let not_found =
                || Response::builder().status(404).body(Vec::new()).unwrap();

            if let Some(width) = width {
                match crate::ui::thumbs::thumbnail_jpeg(&decoded, width) {
                    Some(jpeg) => responder.respond(
                        Response::builder()
                            .header("Content-Type", "image/jpeg")
                            .body(jpeg.as_ref().clone())
                            .unwrap(),
                    ),
                    None => responder.respond(not_found()),
                }
                return;
            }

            match std::fs::read(&decoded) {
                Ok(data) => {
                    let mime = match std::path::Path::new(&decoded)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_ascii_lowercase())
                        .as_deref()
                    {
                        Some("png") => "image/png",
                        Some("gif") => "image/gif",
                        Some("webp") => "image/webp",
                        Some("bmp") => "image/bmp",
                        _ => "image/jpeg",
                    };
                    responder.respond(
                        Response::builder()
                            .header("Content-Type", mime)
                            .body(data)
                            .unwrap(),
                    );
                }
                Err(_) => responder.respond(not_found()),
            }
        });
    });

    // --- Windows tray events arrive through dioxus's global handlers ---
    #[cfg(windows)]
    {
        let tray_for_menu = tray.clone();
        let tx = init.action_tx.clone();
        dioxus::desktop::use_tray_menu_event_handler(move |event| {
            if let Some(action) = tray_for_menu.map_menu_event(&event.id) {
                let _ = tx.send(action);
            }
        });
        let tx = init.action_tx.clone();
        dioxus::desktop::use_tray_icon_event_handler(move |event| {
            use dioxus::desktop::trayicon::TrayIconEvent;
            match event {
                TrayIconEvent::DoubleClick { .. } | TrayIconEvent::Click { .. } => {
                    let _ = tx.send(TrayAction::ShowWindow);
                }
                _ => {}
            }
        });
    }

    // --- Close-to-hide: dioxus hides the window; we track it and notify ---
    {
        let mut window_visible = window_visible;
        use_wry_event_handler(move |event, _| {
            use dioxus::desktop::tao::event::{Event, WindowEvent};
            if let Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } = event
            {
                window_visible.set(false);
                let _ = notify_rust::Notification::new()
                    .summary("Wallpaper Helper")
                    .body("Still running in the system tray.")
                    .show();
            }
        });
    }

    // --- Tray / single-instance actions + scheduler ---
    // Both run via background::spawn_repeating (glib timers on Linux), NOT as
    // VirtualDom futures: those freeze while the window is hidden because the
    // suspended webview can't ack render edits.
    {
        let ctx = ctx.clone();
        let window = use_window();
        let action_rx = init.action_rx.clone();
        use_hook(move || {
            let mut rx = action_rx
                .lock()
                .expect("action receiver lock poisoned")
                .take()
                .expect("action receiver already taken");

            let drain_ctx = ctx.clone();
            let drain_window = window.clone();
            let _ = crate::ui::background::spawn_repeating(
                std::time::Duration::from_millis(100),
                move || {
                    while let Ok(action) = rx.try_recv() {
                        handle_tray_action(&drain_ctx, &drain_window, action);
                    }
                    true
                },
            );

            let mut manager = ctx.manager;
            let _ = crate::ui::background::spawn_repeating(
                std::time::Duration::from_secs(30),
                move || {
                    if let Some(name) = manager.write().check_and_apply_schedule() {
                        log::info!("Scheduler applied profile: {}", name);
                    }
                    true
                },
            );
        });
    }

    let current_view = *view.read();

    rsx! {
        style { {include_str!("../../assets/main.css")} }
        div { class: "app",
            Sidebar {}
            main { class: "content",
                match current_view {
                    View::Profiles => rsx! { ProfilesView {} },
                    View::Collections => rsx! { CollectionsView {} },
                    View::Monitors => rsx! { MonitorsView {} },
                }
            }
            NameModal {}
            ConfirmModal {}
        }
    }
}

/// Handle a tray menu / single-instance action. Runs on the main thread from
/// the 100ms drain tick.
fn handle_tray_action(
    ctx: &AppCtx,
    window: &dioxus::desktop::DesktopContext,
    action: TrayAction,
) {
    log::info!("Tray action received: {:?}", action);
    match action {
        TrayAction::ShowWindow => {
            window.set_visible(true);
            window.set_focus();
            ctx.window_visible.clone().set(true);
        }
        TrayAction::Quit => {
            // CloseWindow events are routed through the close behaviour, so
            // WindowHides would swallow the quit.
            window.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
            window.close();
        }
        TrayAction::NextWallpaper => {
            let col = ctx.slideshow.peek().collection.clone();
            if let Some(col_name) = col {
                ctx.apply_in_collection(&col_name, CollectionStep::Next);
            }
        }
        TrayAction::PrevWallpaper => {
            let col = ctx.slideshow.peek().collection.clone();
            if let Some(col_name) = col {
                ctx.apply_in_collection(&col_name, CollectionStep::Prev);
            }
        }
        TrayAction::ToggleSlideshow => {
            if ctx.slideshow.peek().is_running() {
                ctx.pause_slideshow();
            } else {
                let resume = {
                    let state = ctx.slideshow.peek();
                    state
                        .collection
                        .clone()
                        .map(|col| (col, state.interval_min))
                };
                if let Some((col_name, interval_min)) = resume {
                    ctx.start_slideshow(col_name, interval_min);
                }
            }
        }
    }
}
