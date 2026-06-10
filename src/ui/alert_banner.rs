use dioxus::prelude::*;

use crate::ui::app::DerivedState;

/// Replaces the GTK AlertBox: a warning banner shown when the displayed
/// profile has alias mismatches or missing wallpaper files.
#[component]
pub fn AlertBanner() -> Element {
    let derived = use_context::<DerivedState>();
    let warning = derived.warning.read().clone();

    rsx! {
        if let Some(text) = warning {
            div { class: "alert-box alert-warning",
                div { class: "alert-header", "\u{26a0} Warning" }
                div { class: "alert-body", "{text}" }
            }
        }
    }
}
