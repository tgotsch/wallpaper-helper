use std::collections::HashMap;

use dioxus::prelude::*;

use crate::ui::{wallpaper_thumb_url, AppCtx};

/// Editor tiles cap their decode at this width; large enough to look sharp,
/// small enough that a 4K source doesn't get shipped to the webview whole.
const EDITOR_PREVIEW_WIDTH: u32 = 1024;

/// Strip thumbnails render up to ~120px wide at 48px tall (2x for hidpi).
const STRIP_THUMB_WIDTH: u32 = 240;

/// Large per-monitor preview grid used by the profile editor. Shows the draft
/// wallpapers; tiles are clickable to pick a new image.
#[component]
pub fn EditorGrid() -> Element {
    let ctx = use_context::<AppCtx>();

    let alias_monitors = ctx.manager.clone().read().get_alias_monitor_info();
    let draft = ctx.draft.clone().read().clone();
    let window_visible = *ctx.window_visible.clone().read();

    rsx! {
        div { class: "editor-grid",
            for (alias, monitor_info) in alias_monitors {
                {
                    let (label_text, connected) = match &monitor_info {
                        Some(info) => (format!("{} ({}x{})", alias, info.width, info.height), true),
                        None => (format!("{} (not connected)", alias), false),
                    };

                    let path = draft.get(&alias).cloned().unwrap_or_default();
                    let has_image = !path.is_empty() && std::path::Path::new(&path).exists();
                    let file_name = std::path::Path::new(&path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let ctx_for_click = ctx.clone();
                    let alias_for_click = alias.clone();

                    rsx! {
                        div { class: "editor-tile", key: "{alias}",
                            div { class: "tile-label",
                                span { class: if connected { "tile-alias" } else { "tile-alias disconnected" },
                                    "{label_text}"
                                }
                            }
                            button {
                                class: "tile-image-button",
                                title: "Click to change wallpaper",
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
                                            ctx.draft.clone().write().insert(alias, path);
                                        }
                                    }
                                },
                                if window_visible && has_image {
                                    img {
                                        class: "tile-image",
                                        decoding: "async",
                                        src: "{wallpaper_thumb_url(&path, EDITOR_PREVIEW_WIDTH)}",
                                    }
                                } else {
                                    div { class: "tile-placeholder", "No wallpaper" }
                                }
                                div { class: "tile-hover-overlay", "Change wallpaper" }
                            }
                            if has_image {
                                div { class: "tile-filename", title: "{path}", "{file_name}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Small horizontal strip of a profile's wallpapers (one mini thumbnail per
/// alias). Used in profile cards, collection member rows, and add-previews.
#[component]
pub fn PreviewStrip(profile_name: String) -> Element {
    let ctx = use_context::<AppCtx>();
    let manager = ctx.manager.clone();
    let mgr = manager.read();
    let window_visible = *ctx.window_visible.clone().read();

    let tiles: Vec<(String, Option<String>)> = match mgr.profiles.get(&profile_name) {
        Some(profile) => {
            let paths: HashMap<&String, String> = profile
                .monitor_wallpapers
                .iter()
                .map(|(alias, rel)| (alias, mgr.resolve_wallpaper_path(rel)))
                .collect();
            mgr.aliases
                .iter()
                .map(|alias| {
                    let path = paths
                        .get(alias)
                        .filter(|p| !p.is_empty() && std::path::Path::new(p).exists())
                        .cloned();
                    (alias.clone(), path)
                })
                .collect()
        }
        None => Vec::new(),
    };
    drop(mgr);

    // Each thumb may use at most an equal share of the row, so the strip
    // never overflows the card regardless of monitor count.
    let count = tiles.len().max(1);
    let gaps = (count - 1) * 4;
    let thumb_cap = format!("max-width: calc((100% - {gaps}px) / {count});");

    rsx! {
        div { class: "preview-strip",
            for (alias, path) in tiles {
                if window_visible {
                    if let Some(p) = path {
                        img {
                            key: "{alias}",
                            class: "strip-thumb",
                            style: "{thumb_cap}",
                            title: "{alias}",
                            loading: "lazy",
                            decoding: "async",
                            src: "{wallpaper_thumb_url(&p, STRIP_THUMB_WIDTH)}",
                        }
                    } else {
                        div {
                            key: "{alias}",
                            class: "strip-thumb empty",
                            style: "{thumb_cap}",
                            title: "{alias}",
                        }
                    }
                } else {
                    div {
                        key: "{alias}",
                        class: "strip-thumb empty",
                        style: "{thumb_cap}",
                        title: "{alias}",
                    }
                }
            }
        }
    }
}
