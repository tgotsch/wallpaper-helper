use dioxus::prelude::*;

use crate::ui::app::DerivedState;
use crate::ui::AppCtx;

#[component]
pub fn ProfileSelector() -> Element {
    let ctx = use_context::<AppCtx>();
    let derived = use_context::<DerivedState>();

    let entries = derived.entries.read().clone();
    let selected_index = ctx
        .selected
        .read()
        .as_ref()
        .and_then(|sel| entries.iter().position(|e| e == sel));

    let on_change = {
        let ctx = ctx.clone();
        move |evt: Event<FormData>| {
            let entries = derived.entries.read();
            if let Some(entry) = evt
                .value()
                .parse::<usize>()
                .ok()
                .and_then(|idx| entries.get(idx).cloned())
            {
                ctx.select_entry(Some(entry));
            }
        }
    };

    rsx! {
        div { class: "selector-row",
            select { class: "profile-selector", onchange: on_change,
                for (i, entry) in entries.iter().enumerate() {
                    option {
                        key: "{entry.display_label()}",
                        value: "{i}",
                        selected: selected_index == Some(i),
                        "{entry.display_label()}"
                    }
                }
            }
            button {
                onclick: {
                    let ctx = ctx.clone();
                    move |_| ctx.show_new_profile_dialog.clone().set(true)
                },
                "New profile"
            }
            button {
                onclick: {
                    let ctx = ctx.clone();
                    move |_| ctx.show_collections_modal.clone().set(true)
                },
                "Collections..."
            }
        }
    }
}
