use dioxus::prelude::*;

use crate::ui::{AppCtx, DropdownEntry};

/// Modal dialog for creating a profile from the wallpapers currently shown in
/// the grid (initial wallpapers plus any picker choices).
#[component]
pub fn NewProfileDialog() -> Element {
    let ctx = use_context::<AppCtx>();
    let mut name = use_signal(String::new);

    let close = {
        let ctx = ctx.clone();
        move |_| ctx.show_new_profile_dialog.clone().set(false)
    };

    let create = {
        let ctx = ctx.clone();
        move |_| {
            let profile_name = name.read().trim().to_string();
            if profile_name.is_empty() {
                return;
            }
            log::info!("Creating profile: {}", profile_name);
            {
                let mut manager = ctx.manager;
                let mut mgr = manager.write();
                mgr.create_profile(&profile_name);
                for (alias, path) in ctx.pending.peek().iter() {
                    mgr.set_wallpaper_in_profile(&profile_name, alias, path);
                }
                mgr.save_config(&ctx.config_path);
            }
            ctx.select_entry(Some(DropdownEntry::Profile(profile_name)));
            ctx.show_new_profile_dialog.clone().set(false);
        }
    };

    rsx! {
        div { class: "modal-backdrop",
            div { class: "modal-panel",
                h3 { "Create New Profile" }
                label { "Profile Name:" }
                input {
                    r#type: "text",
                    placeholder: "Enter profile name",
                    value: "{name}",
                    autofocus: true,
                    oninput: move |evt| name.set(evt.value()),
                }
                div { class: "modal-actions",
                    button { onclick: close, "Cancel" }
                    button { onclick: create, "Create" }
                }
            }
        }
    }
}
