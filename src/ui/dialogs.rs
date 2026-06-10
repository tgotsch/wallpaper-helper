use dioxus::prelude::*;

use crate::ui::{AppCtx, ConfirmKind, NameModalKind, View};

/// Shared name-input modal for "new profile", "save profile as", and
/// "new collection". Which one is open lives in `ctx.name_modal`.
#[component]
pub fn NameModal() -> Element {
    let ctx = use_context::<AppCtx>();
    let mut name = use_signal(String::new);

    let kind = ctx.name_modal.clone().read().clone();
    let Some(kind) = kind else {
        // Reset the input whenever the modal is closed.
        if !name.peek().is_empty() {
            name.set(String::new());
        }
        return rsx! {};
    };

    let (title, placeholder, action_label) = match kind {
        NameModalKind::NewProfile => ("Create New Profile", "Profile name", "Create"),
        NameModalKind::SaveProfileAs => ("Save Profile As", "New profile name", "Save"),
        NameModalKind::NewCollection => ("Create New Collection", "Collection name", "Create"),
    };

    let close = {
        let ctx = ctx.clone();
        move |_| ctx.name_modal.clone().set(None)
    };

    let submit = {
        let ctx = ctx.clone();
        let kind = kind.clone();
        move |_| {
            let value = name.read().trim().to_string();
            if value.is_empty() {
                return;
            }
            match kind {
                NameModalKind::NewProfile => {
                    log::info!("Creating profile: {}", value);
                    // Seed the new profile from the wallpapers currently on
                    // the desktop, then open it in the editor.
                    {
                        let mut manager = ctx.manager;
                        let mut mgr = manager.write();
                        if !mgr.create_profile(&value) {
                            return;
                        }
                        let aliases = mgr.aliases.clone();
                        for alias in &aliases {
                            let current = mgr.get_current_wallpaper_by_alias(alias);
                            if !current.is_empty() {
                                mgr.set_wallpaper_in_profile(&value, alias, &current);
                            }
                        }
                        mgr.save_config(&ctx.config_path);
                    }
                    ctx.select_profile(Some(value));
                    ctx.view.clone().set(View::Profiles);
                }
                NameModalKind::SaveProfileAs => {
                    if ctx.manager.clone().read().profiles.contains_key(&value) {
                        log::warn!("Profile '{}' already exists", value);
                        return;
                    }
                    ctx.save_draft_to(&value);
                    ctx.select_profile(Some(value));
                }
                NameModalKind::NewCollection => {
                    log::info!("Creating collection: {}", value);
                    {
                        let mut manager = ctx.manager;
                        let mut mgr = manager.write();
                        if !mgr.create_collection(&value) {
                            return;
                        }
                        mgr.save_config(&ctx.config_path);
                    }
                    ctx.selected_collection.clone().set(Some(value));
                    ctx.view.clone().set(View::Collections);
                }
            }
            ctx.name_modal.clone().set(None);
        }
    };

    rsx! {
        div { class: "modal-backdrop",
            div { class: "modal-panel",
                h3 { "{title}" }
                input {
                    r#type: "text",
                    placeholder: "{placeholder}",
                    value: "{name}",
                    autofocus: true,
                    oninput: move |evt| name.set(evt.value()),
                }
                div { class: "modal-actions",
                    button { class: "btn", onclick: close, "Cancel" }
                    button { class: "btn primary", onclick: submit, "{action_label}" }
                }
            }
        }
    }
}

/// Shared confirmation modal for destructive actions.
#[component]
pub fn ConfirmModal() -> Element {
    let ctx = use_context::<AppCtx>();

    let kind = ctx.confirm_modal.clone().read().clone();
    let Some(kind) = kind else {
        return rsx! {};
    };

    let message = match &kind {
        ConfirmKind::DeleteProfile(name) => format!("Delete profile \u{201c}{}\u{201d}?", name),
        ConfirmKind::DeleteCollection(name) => {
            format!("Delete collection \u{201c}{}\u{201d}? Its profiles are not deleted.", name)
        }
        ConfirmKind::DeleteAlias(alias) => {
            let referencing = ctx
                .manager
                .clone()
                .read()
                .profiles
                .values()
                .filter(|p| p.monitor_wallpapers.contains_key(alias))
                .count();
            if referencing > 0 {
                format!(
                    "Delete monitor alias \u{201c}{}\u{201d}? {} profile(s) still reference it.",
                    alias, referencing
                )
            } else {
                format!("Delete monitor alias \u{201c}{}\u{201d}?", alias)
            }
        }
    };

    let close = {
        let ctx = ctx.clone();
        move |_| ctx.confirm_modal.clone().set(None)
    };

    let confirm = {
        let ctx = ctx.clone();
        let kind = kind.clone();
        move |_| {
            match &kind {
                ConfirmKind::DeleteProfile(name) => {
                    let mut manager = ctx.manager;
                    let mut mgr = manager.write();
                    mgr.delete_profile(name);
                    mgr.save_config(&ctx.config_path);
                    // The selection-fallback effect in App picks a new profile.
                }
                ConfirmKind::DeleteCollection(name) => {
                    if ctx.slideshow.peek().collection.as_deref() == Some(name.as_str()) {
                        ctx.stop_slideshow();
                    }
                    let mut manager = ctx.manager;
                    let mut mgr = manager.write();
                    mgr.delete_collection(name);
                    mgr.save_config(&ctx.config_path);
                }
                ConfirmKind::DeleteAlias(alias) => {
                    {
                        let mut manager = ctx.manager;
                        let mut mgr = manager.write();
                        mgr.current_platform_config_mut().monitor_map.remove(alias);
                        mgr.sync_aliases();
                        mgr.save_config(&ctx.config_path);
                    }
                    // Aliases changed; re-seed the editor draft.
                    let selected = ctx.selected_profile.clone().peek().clone();
                    ctx.select_profile(selected);
                }
            }
            ctx.confirm_modal.clone().set(None);
        }
    };

    rsx! {
        div { class: "modal-backdrop",
            div { class: "modal-panel",
                h3 { "Are you sure?" }
                p { class: "confirm-message", "{message}" }
                div { class: "modal-actions",
                    button { class: "btn", onclick: close, "Cancel" }
                    button { class: "btn danger", onclick: confirm, "Delete" }
                }
            }
        }
    }
}
