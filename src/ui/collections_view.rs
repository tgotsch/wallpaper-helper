use dioxus::prelude::*;

use crate::ui::{
    current_collection_profile, preview::PreviewStrip, sorted_collections, AppCtx,
    CollectionStep, ConfirmKind, NameModalKind,
};

#[component]
pub fn CollectionsView() -> Element {
    let ctx = use_context::<AppCtx>();

    let manager = ctx.manager.clone();
    let collections = sorted_collections(&manager.read());
    let selected = ctx.selected_collection.clone().read().clone();

    rsx! {
        div { class: "view collections-view",
            header { class: "view-header",
                h2 { "Collections" }
                button {
                    class: "btn primary",
                    onclick: {
                        let ctx = ctx.clone();
                        move |_| ctx.name_modal.clone().set(Some(NameModalKind::NewCollection))
                    },
                    "+ New collection"
                }
            }
            div { class: "collections-layout",
                div { class: "collection-list",
                    for name in collections {
                        {
                            let is_selected = selected.as_deref() == Some(name.as_str());
                            let count = manager
                                .read()
                                .collections
                                .get(&name)
                                .map(|c| c.profiles.len())
                                .unwrap_or(0);
                            let mut sel_signal = ctx.selected_collection;
                            let name_for_click = name.clone();
                            rsx! {
                                button {
                                    key: "{name}",
                                    class: if is_selected { "collection-card selected" } else { "collection-card" },
                                    onclick: move |_| sel_signal.set(Some(name_for_click.clone())),
                                    span { class: "collection-card-name", "{name}" }
                                    span { class: "badge count", "{count}" }
                                }
                            }
                        }
                    }
                }
                div { class: "collection-detail",
                    if let Some(col_name) = selected {
                        CollectionDetail { col_name }
                    } else {
                        div { class: "empty-state",
                            p { "No collections yet." }
                            p { class: "empty-hint", "Group profiles into a collection to cycle through them." }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CollectionDetail(col_name: String) -> Element {
    let ctx = use_context::<AppCtx>();
    let mut add_choice: Signal<Option<String>> = use_signal(|| None);

    let manager = ctx.manager.clone();
    let mgr = manager.read();

    // Members in stored (cycle) order.
    let members: Vec<(String, bool)> = mgr
        .collections
        .get(&col_name)
        .map(|c| {
            c.profiles
                .iter()
                .map(|p| (p.clone(), mgr.profiles.contains_key(p)))
                .collect()
        })
        .unwrap_or_default();

    // Profiles that can still be added.
    let available: Vec<String> = {
        let existing: Vec<&String> = members.iter().map(|(p, _)| p).collect();
        let mut avail: Vec<String> = mgr
            .profiles
            .keys()
            .filter(|p| !existing.contains(p))
            .cloned()
            .collect();
        avail.sort();
        avail
    };

    let current_profile = current_collection_profile(&mgr, &col_name);
    drop(mgr);

    let slideshow = ctx.slideshow.clone();
    let slideshow_state = slideshow.read();
    let running_here = slideshow_state.is_running()
        && slideshow_state.collection.as_deref() == Some(col_name.as_str());
    let interval_min = slideshow_state.interval_min;
    drop(slideshow_state);

    let status = ctx.status.clone().read().clone();

    // Candidate for the add-with-preview flow: keep it valid.
    let candidate = add_choice
        .read()
        .clone()
        .filter(|c| available.contains(c))
        .or_else(|| available.first().cloned());

    let step = |direction: CollectionStep| {
        let ctx = ctx.clone();
        let col = col_name.clone();
        move |_| ctx.apply_in_collection(&col, direction)
    };

    rsx! {
        div { class: "collection-pane",
            div { class: "editor-title-row",
                h3 { class: "editor-title", "{col_name}" }
                span { class: "actions-spacer" }
                button {
                    class: "btn danger ghost",
                    onclick: {
                        let ctx = ctx.clone();
                        let name = col_name.clone();
                        move |_| {
                            ctx.confirm_modal
                                .clone()
                                .set(Some(ConfirmKind::DeleteCollection(name.clone())))
                        }
                    },
                    "Delete"
                }
            }

            div { class: "cycle-bar",
                button { class: "btn", onclick: step(CollectionStep::Prev), "\u{2190} Prev" }
                button { class: "btn", onclick: step(CollectionStep::Random), "\u{1f500} Random" }
                button { class: "btn", onclick: step(CollectionStep::Next), "Next \u{2192}" }
                span { class: "cycle-divider" }
                label { class: "interval-label",
                    "Every"
                    input {
                        r#type: "number",
                        min: "1",
                        max: "120",
                        step: "1",
                        value: "{interval_min}",
                        disabled: running_here,
                        oninput: {
                            let ctx = ctx.clone();
                            move |evt: Event<FormData>| {
                                if let Ok(v) = evt.value().parse::<u32>() {
                                    ctx.slideshow.clone().write().interval_min = v.clamp(1, 120);
                                }
                            }
                        },
                    }
                    "min"
                }
                button {
                    class: if running_here { "btn" } else { "btn primary" },
                    onclick: {
                        let ctx = ctx.clone();
                        let col = col_name.clone();
                        move |_| {
                            if running_here {
                                ctx.stop_slideshow();
                            } else {
                                // Starting here takes over any slideshow on
                                // another collection.
                                let interval = ctx.slideshow.peek().interval_min;
                                ctx.start_slideshow(col.clone(), interval);
                            }
                        }
                    },
                    if running_here { "\u{23f9} Stop slideshow" } else { "\u{25b6} Start slideshow" }
                }
            }

            if !status.is_empty() {
                div { class: "status-line", "{status}" }
            }

            div { class: "member-list",
                if members.is_empty() {
                    div { class: "empty-state",
                        p { "This collection is empty." }
                        p { class: "empty-hint", "Add profiles below to start cycling." }
                    }
                }
                for (profile, exists) in members {
                    {
                        let is_current = current_profile.as_deref() == Some(profile.as_str());
                        let ctx_for_remove = ctx.clone();
                        let col_for_remove = col_name.clone();
                        let profile_for_remove = profile.clone();
                        rsx! {
                            div {
                                key: "{profile}",
                                class: if is_current { "member-row current" } else { "member-row" },
                                div { class: "member-info",
                                    div { class: "member-name-row",
                                        span { class: "member-name", "{profile}" }
                                        if !exists {
                                            span { class: "badge missing", "missing" }
                                        }
                                        if is_current {
                                            span { class: "badge current", "current" }
                                        }
                                    }
                                    if exists {
                                        PreviewStrip { profile_name: profile.clone() }
                                    }
                                }
                                button {
                                    class: "btn ghost remove-btn",
                                    title: "Remove from collection",
                                    onclick: move |_| {
                                        let mut manager = ctx_for_remove.manager;
                                        let mut mgr = manager.write();
                                        mgr.remove_profile_from_collection(
                                            &col_for_remove,
                                            &profile_for_remove,
                                        );
                                        mgr.save_config(&ctx_for_remove.config_path);
                                    },
                                    "\u{2715}"
                                }
                            }
                        }
                    }
                }
            }

            if !available.is_empty() {
                div { class: "add-profile-bar",
                    div { class: "add-profile-row",
                        span { class: "add-label", "Add profile:" }
                        select {
                            onchange: move |evt: Event<FormData>| add_choice.set(Some(evt.value())),
                            for profile in available.iter() {
                                option {
                                    key: "{profile}",
                                    value: "{profile}",
                                    selected: candidate.as_deref() == Some(profile.as_str()),
                                    "{profile}"
                                }
                            }
                        }
                        button {
                            class: "btn primary",
                            onclick: {
                                let ctx = ctx.clone();
                                let col = col_name.clone();
                                let candidate = candidate.clone();
                                move |_| {
                                    if let Some(profile) = &candidate {
                                        let mut manager = ctx.manager;
                                        let mut mgr = manager.write();
                                        mgr.add_profile_to_collection(&col, profile);
                                        mgr.save_config(&ctx.config_path);
                                    }
                                }
                            },
                            "Add"
                        }
                    }
                    if let Some(profile) = candidate.clone() {
                        div { class: "add-preview",
                            span { class: "add-preview-label", "Preview:" }
                            PreviewStrip { profile_name: profile }
                        }
                    }
                }
            }
        }
    }
}
