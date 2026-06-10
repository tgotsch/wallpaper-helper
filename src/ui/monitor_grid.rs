use dioxus::prelude::*;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::ui::app::DerivedState;
use crate::ui::{AppCtx, DropdownEntry};

#[component]
pub fn MonitorGrid() -> Element {
    let ctx = use_context::<AppCtx>();
    let derived = use_context::<DerivedState>();

    let alias_monitors = ctx.manager.read().get_alias_monitor_info();
    let displayed_profile = derived.displayed_profile.read().clone();
    let overrides = ctx.display_overrides.read().clone();
    let is_collection = matches!(&*ctx.selected.read(), Some(DropdownEntry::Collection(_)));
    let window_visible = *ctx.window_visible.read();

    rsx! {
        div { class: "monitor-grid",
            for (alias, monitor_info) in alias_monitors {
                {
                    let label_text = match &monitor_info {
                        Some(info) => format!("{} ({}x{})", alias, info.width, info.height),
                        None => format!("{} (not connected)", alias),
                    };

                    // Picker overrides win; otherwise show the displayed
                    // profile's wallpaper if the file exists.
                    let image_path: Option<String> = overrides.get(&alias).cloned().or_else(|| {
                        let mgr = ctx.manager.read();
                        displayed_profile
                            .as_ref()
                            .and_then(|p| mgr.profiles.get(p))
                            .and_then(|p| p.monitor_wallpapers.get(&alias))
                            .map(|rel| mgr.resolve_wallpaper_path(rel))
                            .filter(|abs| !abs.is_empty() && std::path::Path::new(abs).exists())
                    });

                    let ctx_for_click = ctx.clone();
                    let alias_for_click = alias.clone();

                    rsx! {
                        div { class: "monitor-tile", key: "{alias}",
                            div { class: "monitor-label", "{label_text}" }
                            button {
                                class: "monitor-image-button",
                                disabled: is_collection,
                                onclick: move |_| {
                                    let ctx = ctx_for_click.clone();
                                    let alias = alias_for_click.clone();
                                    async move {
                                        let picked = rfd::AsyncFileDialog::new()
                                            .set_title("Select Wallpaper")
                                            .add_filter(
                                                "Image files",
                                                &["jpg", "jpeg", "png", "gif", "bmp", "webp"],
                                            )
                                            .pick_file()
                                            .await;
                                        if let Some(file) = picked {
                                            let path = file.path().to_string_lossy().to_string();
                                            ctx.pending.clone().write().insert(alias.clone(), path.clone());
                                            ctx.display_overrides.clone().write().insert(alias, path);
                                        }
                                    }
                                },
                                if window_visible {
                                    if let Some(path) = image_path {
                                        img {
                                            class: "monitor-image",
                                            src: "/wallpaper/{utf8_percent_encode(&path, NON_ALPHANUMERIC)}",
                                        }
                                    } else {
                                        div { class: "monitor-image-placeholder", "No wallpaper" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
