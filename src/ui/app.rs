use std::collections::HashMap;
use std::rc::Rc;

use dioxus::desktop::{use_asset_handler, use_window, use_wry_event_handler};
use dioxus::prelude::*;
use percent_encoding::percent_decode_str;

use crate::tray::{AppTray, TrayAction};
use crate::ui::{
    alert_banner::AlertBanner, collection_controls::CollectionControls,
    collections_modal::CollectionsModal, dialogs::NewProfileDialog, displayed_profile_name,
    monitor_grid::MonitorGrid, profile_selector::ProfileSelector, warning_for_profile, AppCtx,
    AppInit, CollectionStep, DropdownEntry, SlideshowState,
};
use crate::wallpaper_manager::WallpaperManager;

/// Extra context the components read alongside AppCtx: derived (memoized) state.
#[derive(Clone, Copy)]
pub struct DerivedState {
    pub entries: Memo<Vec<DropdownEntry>>,
    pub displayed_profile: Memo<Option<String>>,
    pub warning: Memo<Option<String>>,
}

#[component]
pub fn App() -> Element {
    let init = use_context::<AppInit>();

    let manager = use_signal(|| {
        let mut m = WallpaperManager::new();
        m.load_config(&init.config_path);
        m
    });

    // Seed per-alias paths from the wallpapers active at startup (same as the
    // GTK version: these feed "New profile" until overwritten by the picker).
    let pending = use_signal(|| {
        let mgr = manager.peek();
        let mut map = HashMap::new();
        for (alias, _) in mgr.get_alias_monitor_info() {
            map.insert(alias.clone(), mgr.get_current_wallpaper_by_alias(&alias));
        }
        map
    });

    let selected = use_signal(|| {
        let mgr = manager.peek();
        let mut profile_names: Vec<String> = mgr.profiles.keys().cloned().collect();
        profile_names.sort();
        if profile_names.iter().any(|n| n == "default") {
            return Some(DropdownEntry::Profile("default".to_string()));
        }
        if let Some(first) = profile_names.first() {
            return Some(DropdownEntry::Profile(first.clone()));
        }
        let mut collection_names: Vec<String> = mgr.collections.keys().cloned().collect();
        collection_names.sort();
        collection_names
            .first()
            .map(|n| DropdownEntry::Collection(n.clone()))
    });

    let display_overrides = use_signal(HashMap::new);
    let status = use_signal(String::new);
    let slideshow = use_signal(SlideshowState::default);
    let show_collections_modal = use_signal(|| false);
    let show_new_profile_dialog = use_signal(|| false);
    let window_visible = use_signal(|| true);

    let tray = use_hook(|| Rc::new(AppTray::new(init.action_tx.clone())));

    let ctx = use_context_provider(|| AppCtx {
        manager,
        config_path: Rc::from(init.config_path.as_str()),
        selected,
        pending,
        display_overrides,
        status,
        slideshow,
        show_collections_modal,
        show_new_profile_dialog,
        window_visible,
        tray: tray.clone(),
    });

    // --- Derived state ---
    let entries = use_memo(move || {
        let mgr = manager.read();
        let mut profile_names: Vec<String> = mgr.profiles.keys().cloned().collect();
        profile_names.sort();
        let mut collection_names: Vec<String> = mgr.collections.keys().cloned().collect();
        collection_names.sort();
        let mut entries: Vec<DropdownEntry> = profile_names
            .into_iter()
            .map(DropdownEntry::Profile)
            .collect();
        entries.extend(collection_names.into_iter().map(DropdownEntry::Collection));
        entries
    });

    let displayed_profile =
        use_memo(move || displayed_profile_name(&manager.read(), &selected.read()));

    let warning = use_memo(move || {
        let mgr = manager.read();
        match &*selected.read() {
            Some(DropdownEntry::Profile(name)) => warning_for_profile(&mgr, name),
            Some(DropdownEntry::Collection(col_name)) => {
                if mgr.get_valid_collection_profiles(col_name).is_empty() {
                    Some("Collection has no valid profiles".to_string())
                } else {
                    displayed_profile
                        .read()
                        .as_ref()
                        .and_then(|p| warning_for_profile(&mgr, p))
                }
            }
            None => None,
        }
    });

    use_context_provider(|| DerivedState {
        entries,
        displayed_profile,
        warning,
    });

    // Keep the selection valid when profiles/collections change (e.g. a
    // collection is deleted while selected).
    {
        let ctx = ctx.clone();
        use_effect(move || {
            let entries = entries.read();
            let still_valid = selected
                .peek()
                .as_ref()
                .is_some_and(|sel| entries.contains(sel));
            if !still_valid {
                let fallback = entries
                    .iter()
                    .find(|e| matches!(e, DropdownEntry::Profile(n) if n == "default"))
                    .cloned()
                    .or_else(|| entries.first().cloned());
                ctx.select_entry(fallback);
            }
        });
    }

    // --- Wallpaper thumbnails: serve local files to the webview ---
    use_asset_handler("wallpaper", move |request, responder| {
        let decoded = percent_decode_str(request.uri().path().trim_start_matches("/wallpaper/"))
            .decode_utf8_lossy()
            .to_string();
        std::thread::spawn(move || {
            use dioxus::desktop::wry::http::Response;
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
                Err(_) => responder.respond(
                    Response::builder().status(404).body(Vec::new()).unwrap(),
                ),
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

    let show_apply = matches!(&*selected.read(), Some(DropdownEntry::Profile(_)));

    rsx! {
        style { {include_str!("../../assets/main.css")} }
        div { class: "app",
            ProfileSelector {}
            AlertBanner {}
            MonitorGrid {}
            if show_apply {
                button {
                    class: "apply-button",
                    onclick: move |_| {
                        let name = match &*selected.peek() {
                            Some(DropdownEntry::Profile(name)) => name.clone(),
                            _ => return,
                        };
                        log::info!("Applying profile: '{}'", name);
                        manager.clone().write().apply_profile(&name);
                    },
                    "Apply Selected profile"
                }
            }
            CollectionControls {}
            if *show_new_profile_dialog.read() {
                NewProfileDialog {}
            }
            if *show_collections_modal.read() {
                CollectionsModal {}
            }
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
