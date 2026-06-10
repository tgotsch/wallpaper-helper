use dioxus::prelude::*;

use crate::ui::{AppCtx, CollectionStep, DropdownEntry};

/// Prev/Random/Next + slideshow controls, shown only while a collection is
/// selected in the dropdown.
#[component]
pub fn CollectionControls() -> Element {
    let ctx = use_context::<AppCtx>();

    let col_name = match &*ctx.selected.read() {
        Some(DropdownEntry::Collection(name)) => name.clone(),
        _ => return rsx! {},
    };

    let running = ctx.slideshow.read().is_running();
    let interval_min = ctx.slideshow.read().interval_min;
    let status = ctx.status.read().clone();

    let step_handler = |step: CollectionStep| {
        let ctx = ctx.clone();
        let col_name = col_name.clone();
        move |_| ctx.apply_in_collection(&col_name, step)
    };

    rsx! {
        div { class: "collection-controls",
            div { class: "collection-action-bar",
                button { onclick: step_handler(CollectionStep::Prev), "Prev" }
                button { onclick: step_handler(CollectionStep::Random), "Random" }
                button { onclick: step_handler(CollectionStep::Next), "Next" }
            }
            div { class: "slideshow-bar",
                span { "Slideshow:" }
                input {
                    r#type: "number",
                    min: "1",
                    max: "120",
                    step: "1",
                    value: "{interval_min}",
                    disabled: running,
                    oninput: {
                        let ctx = ctx.clone();
                        move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u32>() {
                                ctx.slideshow.clone().write().interval_min = v.clamp(1, 120);
                            }
                        }
                    },
                }
                span { "min" }
                button {
                    onclick: {
                        let ctx = ctx.clone();
                        let col_name = col_name.clone();
                        move |_| {
                            if ctx.slideshow.peek().is_running() {
                                ctx.stop_slideshow();
                            } else {
                                let interval_min = ctx.slideshow.peek().interval_min;
                                ctx.start_slideshow(col_name.clone(), interval_min);
                            }
                        }
                    },
                    if running { "Stop" } else { "Start" }
                }
            }
            div { class: "collection-status", "{status}" }
        }
    }
}
