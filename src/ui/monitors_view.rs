use dioxus::prelude::*;

use crate::ui::{AppCtx, ConfirmKind};
use crate::wallpaper_manager::MonitorMapping;

/// Editor for the current platform's monitor configuration: wallpaper base
/// path and the alias -> device map (with optional resolution overrides).
/// Everything that used to require hand-editing config.json.
#[component]
pub fn MonitorsView() -> Element {
    let ctx = use_context::<AppCtx>();
    let mut base_path_input: Signal<Option<String>> = use_signal(|| None);
    let mut new_alias = use_signal(String::new);
    let mut new_device = use_signal(String::new);

    let manager = ctx.manager.clone();
    let mgr = manager.read();

    let saved_base_path = {
        #[cfg(target_os = "linux")]
        { mgr.platform_configs.linux.wallpaper_base_path.clone() }
        #[cfg(windows)]
        { mgr.platform_configs.windows.wallpaper_base_path.clone() }
    };
    let base_path = base_path_input.read().clone().unwrap_or_else(|| saved_base_path.clone());
    let base_path_dirty = base_path != saved_base_path;

    let detected = mgr.monitors.clone();

    // alias -> (device, width_override, height_override, connected)
    let mappings: Vec<(String, String, Option<u32>, Option<u32>, bool)> = {
        let config = {
            #[cfg(target_os = "linux")]
            { &mgr.platform_configs.linux }
            #[cfg(windows)]
            { &mgr.platform_configs.windows }
        };
        let mut aliases: Vec<&String> = config.monitor_map.keys().collect();
        aliases.sort();
        aliases
            .into_iter()
            .map(|alias| {
                let mapping = &config.monitor_map[alias];
                let device = mapping.device().to_string();
                let (w, h) = match mapping.resolution() {
                    Some((w, h)) => (Some(w), Some(h)),
                    None => (None, None),
                };
                let connected = detected.iter().any(|m| m.device_name == device);
                (alias.clone(), device, w, h, connected)
            })
            .collect()
    };

    // device -> alias (for the detected-monitor strip)
    let alias_of_device = |device: &str| -> Option<String> {
        mappings
            .iter()
            .find(|(_, dev, ..)| dev == device)
            .map(|(alias, ..)| alias.clone())
    };
    let detected_rows: Vec<(String, u32, u32, bool, Option<String>)> = detected
        .iter()
        .map(|m| {
            (
                m.device_name.clone(),
                m.width,
                m.height,
                m.is_primary,
                alias_of_device(&m.device_name),
            )
        })
        .collect();
    drop(mgr);

    // Persist an alias mapping (device + optional resolution override).
    let set_mapping = use_callback({
        let ctx = ctx.clone();
        move |(alias, device, w, h): (String, String, Option<u32>, Option<u32>)| {
            let mapping = match (w, h) {
                (Some(width), Some(height)) => MonitorMapping::Detailed {
                    device,
                    width: Some(width),
                    height: Some(height),
                },
                _ => MonitorMapping::Simple(device),
            };
            {
                let mut manager = ctx.manager;
                let mut mgr = manager.write();
                mgr.current_platform_config_mut()
                    .monitor_map
                    .insert(alias, mapping);
                mgr.sync_aliases();
                mgr.save_config(&ctx.config_path);
            }
            // Alias set may have changed; re-seed the profile editor draft.
            let selected = ctx.selected_profile.clone().peek().clone();
            ctx.select_profile(selected);
        }
    });

    rsx! {
        div { class: "view monitors-view",
            header { class: "view-header",
                h2 { "Monitors" }
                button {
                    class: "btn",
                    onclick: {
                        let ctx = ctx.clone();
                        move |_| ctx.manager.clone().write().refresh_monitors()
                    },
                    "\u{21bb} Rescan"
                }
            }

            section { class: "settings-card",
                h4 { "Wallpaper folder" }
                p { class: "settings-hint",
                    "Profiles store paths relative to this folder, so one config can be shared across machines."
                }
                div { class: "settings-row",
                    input {
                        r#type: "text",
                        class: "base-path-input",
                        placeholder: "e.g. /home/user/wallpapers",
                        value: "{base_path}",
                        oninput: move |evt| base_path_input.set(Some(evt.value())),
                    }
                    button {
                        class: "btn primary",
                        disabled: !base_path_dirty,
                        onclick: {
                            let ctx = ctx.clone();
                            move |_| {
                                let value = base_path_input.peek().clone().unwrap_or_default();
                                {
                                    let mut manager = ctx.manager;
                                    let mut mgr = manager.write();
                                    mgr.current_platform_config_mut().wallpaper_base_path = value;
                                    mgr.save_config(&ctx.config_path);
                                }
                                base_path_input.set(None);
                                let selected = ctx.selected_profile.clone().peek().clone();
                                ctx.select_profile(selected);
                            }
                        },
                        "Save"
                    }
                }
            }

            section { class: "settings-card",
                h4 { "Detected monitors" }
                div { class: "detected-strip",
                    if detected_rows.is_empty() {
                        p { class: "settings-hint", "No monitors detected." }
                    }
                    for (device, width, height, primary, mapped) in detected_rows {
                        div { class: "detected-card", key: "{device}",
                            div { class: "detected-name", "{device}" }
                            div { class: "detected-res", "{width}\u{00d7}{height}" }
                            div { class: "detected-badges",
                                if primary {
                                    span { class: "badge primary-badge", "primary" }
                                }
                                if let Some(alias) = mapped {
                                    span { class: "badge mapped", "\u{2192} {alias}" }
                                } else {
                                    span { class: "badge unmapped", "unmapped" }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "settings-card",
                h4 { "Monitor aliases" }
                p { class: "settings-hint",
                    "Profiles reference monitors by these names. Map each alias to a device connector; set a resolution override if Plasma's screen ordering forces you to map an alias to a different connector."
                }
                datalist { id: "detected-devices",
                    for m in detected.iter() {
                        option { key: "{m.device_name}", value: "{m.device_name}" }
                    }
                }
                div { class: "alias-list",
                    for (alias, device, w, h, connected) in mappings {
                        AliasRow {
                            key: "{alias}",
                            alias: alias.clone(),
                            device,
                            width: w,
                            height: h,
                            connected,
                            on_change: set_mapping,
                        }
                    }
                }
                div { class: "alias-add-row",
                    input {
                        r#type: "text",
                        placeholder: "New alias (e.g. main)",
                        value: "{new_alias}",
                        oninput: move |evt| new_alias.set(evt.value()),
                    }
                    input {
                        r#type: "text",
                        list: "detected-devices",
                        placeholder: "Device (e.g. DP-2)",
                        value: "{new_device}",
                        oninput: move |evt| new_device.set(evt.value()),
                    }
                    button {
                        class: "btn primary",
                        onclick: {
                            let ctx = ctx.clone();
                            move |_| {
                                let alias = new_alias.peek().trim().to_string();
                                let device = new_device.peek().trim().to_string();
                                if alias.is_empty() || device.is_empty() {
                                    return;
                                }
                                if ctx
                                    .manager
                                    .clone()
                                    .read()
                                    .aliases
                                    .contains(&alias)
                                {
                                    log::warn!("Alias '{}' already exists", alias);
                                    return;
                                }
                                set_mapping.call((alias, device, None, None));
                                new_alias.set(String::new());
                                new_device.set(String::new());
                            }
                        },
                        "+ Add alias"
                    }
                }
            }
        }
    }
}

#[component]
fn AliasRow(
    alias: String,
    device: String,
    width: Option<u32>,
    height: Option<u32>,
    connected: bool,
    on_change: Callback<(String, String, Option<u32>, Option<u32>)>,
) -> Element {
    let ctx = use_context::<AppCtx>();

    let w_str = width.map(|w| w.to_string()).unwrap_or_default();
    let h_str = height.map(|h| h.to_string()).unwrap_or_default();

    rsx! {
        div { class: "alias-row",
            div { class: "alias-name-cell",
                span { class: "alias-name", "{alias}" }
                span {
                    class: if connected { "badge connected" } else { "badge disconnected" },
                    if connected { "connected" } else { "not connected" }
                }
            }
            label { class: "alias-field",
                span { class: "field-label", "Device" }
                input {
                    r#type: "text",
                    list: "detected-devices",
                    value: "{device}",
                    onchange: {
                        let alias = alias.clone();
                        move |evt: Event<FormData>| {
                            let dev = evt.value().trim().to_string();
                            if !dev.is_empty() {
                                on_change((alias.clone(), dev, width, height));
                            }
                        }
                    },
                }
            }
            label { class: "alias-field res-field",
                span { class: "field-label", "Resolution override" }
                div { class: "res-inputs",
                    input {
                        r#type: "number",
                        min: "1",
                        placeholder: "auto",
                        value: "{w_str}",
                        onchange: {
                            let alias = alias.clone();
                            let device = device.clone();
                            move |evt: Event<FormData>| {
                                let w = evt.value().parse::<u32>().ok();
                                on_change((alias.clone(), device.clone(), w, height));
                            }
                        },
                    }
                    span { class: "res-x", "\u{00d7}" }
                    input {
                        r#type: "number",
                        min: "1",
                        placeholder: "auto",
                        value: "{h_str}",
                        onchange: {
                            let alias = alias.clone();
                            let device = device.clone();
                            move |evt: Event<FormData>| {
                                let h = evt.value().parse::<u32>().ok();
                                on_change((alias.clone(), device.clone(), width, h));
                            }
                        },
                    }
                }
            }
            button {
                class: "btn ghost remove-btn",
                title: "Delete alias",
                onclick: {
                    let ctx = ctx.clone();
                    let alias = alias.clone();
                    move |_| {
                        ctx.confirm_modal
                            .clone()
                            .set(Some(ConfirmKind::DeleteAlias(alias.clone())))
                    }
                },
                "\u{2715}"
            }
        }
    }
}
