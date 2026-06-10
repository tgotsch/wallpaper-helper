use dioxus::prelude::*;

/// Warning banner shown when a profile has alias mismatches or missing
/// wallpaper files.
#[component]
pub fn AlertBanner(text: String) -> Element {
    rsx! {
        div { class: "alert-box alert-warning",
            div { class: "alert-header", "\u{26a0} Warning" }
            div { class: "alert-body", "{text}" }
        }
    }
}
