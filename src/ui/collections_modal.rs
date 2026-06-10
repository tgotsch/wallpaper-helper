use dioxus::prelude::*;

use crate::ui::AppCtx;

/// Modal replacement for the GTK collections window: manage collections and
/// the profiles inside them. Every mutation saves the config immediately.
#[component]
pub fn CollectionsModal() -> Element {
    let ctx = use_context::<AppCtx>();
    let mut selected_collection: Signal<Option<String>> = use_signal(|| None);
    let mut selected_profile: Signal<Option<String>> = use_signal(|| None);
    let mut new_collection_name = use_signal(String::new);
    let mut add_profile_choice = use_signal(String::new);

    let mgr = ctx.manager.read();
    let mut collection_names: Vec<String> = mgr.collections.keys().cloned().collect();
    collection_names.sort();

    let current_collection = selected_collection.read().clone();

    // Profiles of the selected collection (in stored order), marking missing ones.
    let profiles_in_collection: Vec<(String, bool)> = current_collection
        .as_ref()
        .and_then(|col_name| mgr.collections.get(col_name))
        .map(|col| {
            col.profiles
                .iter()
                .map(|p| (p.clone(), mgr.profiles.contains_key(p)))
                .collect()
        })
        .unwrap_or_default();

    // Profiles that can still be added to the selected collection.
    let available_profiles: Vec<String> = current_collection
        .as_ref()
        .map(|col_name| {
            let existing: Vec<String> = mgr
                .collections
                .get(col_name)
                .map(|c| c.profiles.clone())
                .unwrap_or_default();
            let mut available: Vec<String> = mgr
                .profiles
                .keys()
                .filter(|p| !existing.contains(p))
                .cloned()
                .collect();
            available.sort();
            available
        })
        .unwrap_or_default();
    drop(mgr);

    let has_selection = current_collection.is_some();

    let on_create = {
        let ctx = ctx.clone();
        move |_| {
            let name = new_collection_name.read().trim().to_string();
            if name.is_empty() {
                return;
            }
            {
                let mut manager = ctx.manager;
                let mut mgr = manager.write();
                mgr.create_collection(&name);
                mgr.save_config(&ctx.config_path);
            }
            new_collection_name.set(String::new());
            selected_collection.set(Some(name));
            selected_profile.set(None);
        }
    };

    let on_delete = {
        let ctx = ctx.clone();
        move |_| {
            let col_name = match selected_collection.peek().clone() {
                Some(name) => name,
                None => return,
            };
            {
                let mut manager = ctx.manager;
                let mut mgr = manager.write();
                mgr.delete_collection(&col_name);
                mgr.save_config(&ctx.config_path);
            }
            selected_collection.set(None);
            selected_profile.set(None);
        }
    };

    let on_add_profile = {
        let ctx = ctx.clone();
        let available = available_profiles.clone();
        move |_| {
            let col_name = match selected_collection.peek().clone() {
                Some(name) => name,
                None => return,
            };
            // Default to the first available profile if none was picked yet.
            let choice = {
                let chosen = add_profile_choice.peek().clone();
                if available.contains(&chosen) {
                    chosen
                } else {
                    match available.first() {
                        Some(first) => first.clone(),
                        None => return,
                    }
                }
            };
            {
                let mut manager = ctx.manager;
                let mut mgr = manager.write();
                mgr.add_profile_to_collection(&col_name, &choice);
                mgr.save_config(&ctx.config_path);
            }
            add_profile_choice.set(String::new());
        }
    };

    let on_remove_profile = {
        let ctx = ctx.clone();
        move |_| {
            let col_name = match selected_collection.peek().clone() {
                Some(name) => name,
                None => return,
            };
            let profile_name = match selected_profile.peek().clone() {
                Some(name) => name,
                None => return,
            };
            {
                let mut manager = ctx.manager;
                let mut mgr = manager.write();
                mgr.remove_profile_from_collection(&col_name, &profile_name);
                mgr.save_config(&ctx.config_path);
            }
            selected_profile.set(None);
        }
    };

    rsx! {
        div { class: "modal-backdrop",
            div { class: "modal-panel collections-panel",
                div { class: "collections-header",
                    h3 { "Collections" }
                    input {
                        r#type: "text",
                        placeholder: "New collection name",
                        value: "{new_collection_name}",
                        oninput: move |evt| new_collection_name.set(evt.value()),
                    }
                    button { onclick: on_create, "New" }
                    button { disabled: !has_selection, onclick: on_delete, "Delete" }
                    span { class: "spacer" }
                    button {
                        onclick: {
                            let ctx = ctx.clone();
                            move |_| ctx.show_collections_modal.clone().set(false)
                        },
                        "Close"
                    }
                }
                div { class: "collections-content",
                    ul { class: "collection-list",
                        for name in collection_names {
                            {
                                let is_selected = current_collection.as_deref() == Some(name.as_str());
                                let name_for_click = name.clone();
                                rsx! {
                                    li {
                                        key: "{name}",
                                        class: if is_selected { "selected" } else { "" },
                                        onclick: move |_| {
                                            selected_collection.set(Some(name_for_click.clone()));
                                            selected_profile.set(None);
                                        },
                                        "{name}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "collection-detail",
                        div { class: "collection-detail-header",
                            span { class: "collection-name",
                                {current_collection.clone().unwrap_or_else(|| "(select a collection)".to_string())}
                            }
                            if has_selection {
                                select {
                                    onchange: move |evt: Event<FormData>| add_profile_choice.set(evt.value()),
                                    for profile in available_profiles.iter() {
                                        option {
                                            key: "{profile}",
                                            value: "{profile}",
                                            "{profile}"
                                        }
                                    }
                                }
                                button {
                                    disabled: available_profiles.is_empty(),
                                    onclick: on_add_profile,
                                    "Add Profile"
                                }
                                button {
                                    disabled: selected_profile.read().is_none(),
                                    onclick: on_remove_profile,
                                    "Remove"
                                }
                            }
                        }
                        ul { class: "profile-list",
                            for (profile, exists) in profiles_in_collection {
                                {
                                    let display = if exists {
                                        profile.clone()
                                    } else {
                                        format!("{} (missing)", profile)
                                    };
                                    let is_selected = selected_profile.read().as_deref() == Some(profile.as_str());
                                    let profile_for_click = profile.clone();
                                    rsx! {
                                        li {
                                            key: "{profile}",
                                            class: if is_selected { "selected" } else { "" },
                                            onclick: move |_| selected_profile.set(Some(profile_for_click.clone())),
                                            "{display}"
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
}
