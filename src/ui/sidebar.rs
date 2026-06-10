use dioxus::prelude::*;

use crate::ui::{AppCtx, View};

#[component]
pub fn Sidebar() -> Element {
    let ctx = use_context::<AppCtx>();
    let current = *ctx.view.clone().read();

    let nav_item = |target: View, icon: &'static str, label: &'static str| {
        let ctx = ctx.clone();
        let active = current == target;
        rsx! {
            button {
                class: if active { "nav-item active" } else { "nav-item" },
                onclick: move |_| ctx.view.clone().set(target),
                span { class: "nav-icon", "{icon}" }
                "{label}"
            }
        }
    };

    let slideshow = ctx.slideshow.clone();
    let slideshow = slideshow.read();
    let chip = slideshow.collection.as_ref().map(|col| {
        let running = slideshow.is_running();
        (col.clone(), running)
    });

    rsx! {
        nav { class: "sidebar",
            div { class: "app-title",
                span { class: "app-title-icon", "\u{25c9}" }
                "Wallpaper Helper"
            }
            {nav_item(View::Profiles, "\u{1f5bc}", "Profiles")}
            {nav_item(View::Collections, "\u{1f5c2}", "Collections")}
            {nav_item(View::Monitors, "\u{1f5a5}", "Monitors")}
            div { class: "sidebar-spacer" }
            if let Some((col, running)) = chip {
                div {
                    class: if running { "slideshow-chip running" } else { "slideshow-chip paused" },
                    span { class: "chip-dot" }
                    div { class: "chip-text",
                        div { class: "chip-title",
                            if running { "Slideshow" } else { "Slideshow paused" }
                        }
                        div { class: "chip-sub", "{col}" }
                    }
                }
            }
        }
    }
}
