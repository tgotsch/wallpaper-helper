use dioxus::prelude::*;

use crate::ui::{
    alert_banner::AlertBanner, preview::{EditorGrid, PreviewStrip}, sorted_profiles,
    warning_for_profile, AppCtx, ConfirmKind, NameModalKind,
};

#[component]
pub fn ProfilesView() -> Element {
    let ctx = use_context::<AppCtx>();

    let manager = ctx.manager.clone();
    let profiles = sorted_profiles(&manager.read());
    let selected = ctx.selected_profile.clone().read().clone();
    let dirty = ctx.draft_dirty();

    rsx! {
        div { class: "view profiles-view",
            header { class: "view-header",
                h2 { "Profiles" }
                button {
                    class: "btn primary",
                    onclick: {
                        let ctx = ctx.clone();
                        move |_| ctx.name_modal.clone().set(Some(NameModalKind::NewProfile))
                    },
                    "+ New profile"
                }
            }
            div { class: "profiles-layout",
                div { class: "profile-list",
                    for name in profiles {
                        {
                            let is_selected = selected.as_deref() == Some(name.as_str());
                            let has_warning = warning_for_profile(&manager.read(), &name).is_some();
                            let ctx_for_click = ctx.clone();
                            let name_for_click = name.clone();
                            rsx! {
                                button {
                                    key: "{name}",
                                    class: if is_selected { "profile-card selected" } else { "profile-card" },
                                    onclick: move |_| {
                                        ctx_for_click.select_profile(Some(name_for_click.clone()));
                                    },
                                    div { class: "profile-card-header",
                                        span { class: "profile-card-name", "{name}" }
                                        if has_warning {
                                            span { class: "warning-dot", title: "This profile has warnings" }
                                        }
                                    }
                                    PreviewStrip { profile_name: name.clone() }
                                }
                            }
                        }
                    }
                }
                div { class: "profile-editor",
                    if let Some(profile_name) = selected {
                        ProfileEditor { profile_name, dirty }
                    } else {
                        div { class: "empty-state",
                            p { "No profiles yet." }
                            p { class: "empty-hint", "Create one to capture your current wallpapers." }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ProfileEditor(profile_name: String, dirty: bool) -> Element {
    let ctx = use_context::<AppCtx>();

    let warning = warning_for_profile(&ctx.manager.clone().read(), &profile_name);

    let apply = {
        let ctx = ctx.clone();
        let name = profile_name.clone();
        move |_| {
            log::info!("Applying profile: '{}'", name);
            ctx.manager.clone().write().apply_profile(&name);
            ctx.status.clone().set(format!("Applied: {}", name));
        }
    };

    let save = {
        let ctx = ctx.clone();
        let name = profile_name.clone();
        move |_| {
            ctx.save_draft_to(&name);
        }
    };

    let discard = {
        let ctx = ctx.clone();
        let name = profile_name.clone();
        move |_| {
            ctx.select_profile(Some(name.clone()));
        }
    };

    rsx! {
        div { class: "editor-pane",
            div { class: "editor-title-row",
                h3 { class: "editor-title", "{profile_name}" }
                if dirty {
                    span { class: "badge dirty", "unsaved changes" }
                }
            }
            if let Some(text) = warning {
                AlertBanner { text }
            }
            EditorGrid {}
            div { class: "editor-actions",
                button {
                    class: "btn primary",
                    disabled: dirty,
                    title: if dirty { "Save your changes before applying" } else { "Set these wallpapers now" },
                    onclick: apply,
                    "Apply profile"
                }
                if dirty {
                    button { class: "btn primary", onclick: save, "Save changes" }
                    button { class: "btn", onclick: discard, "Discard" }
                }
                button {
                    class: "btn",
                    onclick: {
                        let ctx = ctx.clone();
                        move |_| ctx.name_modal.clone().set(Some(NameModalKind::SaveProfileAs))
                    },
                    "Save as new\u{2026}"
                }
                span { class: "actions-spacer" }
                button {
                    class: "btn danger ghost",
                    onclick: {
                        let ctx = ctx.clone();
                        let name = profile_name.clone();
                        move |_| {
                            ctx.confirm_modal
                                .clone()
                                .set(Some(ConfirmKind::DeleteProfile(name.clone())))
                        }
                    },
                    "Delete"
                }
            }
        }
    }
}
