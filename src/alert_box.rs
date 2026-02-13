use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{Box, Label, CssProvider};
use gtk::gdk::Display;

#[derive(Clone)]
pub struct AlertBox {
    pub container: Box,
    body: Label,
}

pub enum AlertLevel {
    Warning,
    Error,
}

impl AlertLevel {
    fn css_class(&self) -> &'static str {
        match self {
            AlertLevel::Warning => "alert-warning",
            AlertLevel::Error => "alert-error",
        }
    }

    fn header_markup(&self) -> &'static str {
        match self {
            AlertLevel::Warning => "<b>\u{26A0} Warning</b>",
            AlertLevel::Error => "<b>\u{2718} Error</b>",
        }
    }
}

impl AlertBox {
    pub fn new(level: AlertLevel) -> Self {
        let container = Box::new(gtk::Orientation::Vertical, 4);
        container.set_visible(false);
        container.add_css_class("alert-box");
        container.add_css_class(level.css_class());

        let header = Label::new(None);
        header.set_markup(level.header_markup());
        header.set_xalign(0.0);
        header.add_css_class("alert-header");

        let body = Label::new(None);
        body.set_xalign(0.0);
        body.add_css_class("alert-body");

        container.append(&header);
        container.append(&body);

        Self { container, body }
    }

    pub fn show(&self, text: &str) {
        self.body.set_text(text);
        self.container.set_visible(true);
    }

    pub fn hide(&self) {
        self.container.set_visible(false);
    }

    pub fn load_css() {
        let css_provider = CssProvider::new();
        css_provider.load_from_data(
            "box.alert-box { \
                border-radius: 6px; \
                padding: 10px 14px; \
                margin: 4px 0; \
            } \
            box.alert-warning { \
                border-left: 4px solid #9a6700; \
                background-color: alpha(#9a6700, 0.08); \
            } \
            box.alert-warning label.alert-header { \
                color: #9a6700; \
            } \
            box.alert-warning label.alert-body { \
                color: #b5890a; \
            } \
            box.alert-error { \
                border-left: 4px solid #d1242f; \
                background-color: alpha(#d1242f, 0.08); \
            } \
            box.alert-error label.alert-header { \
                color: #d1242f; \
            } \
            box.alert-error label.alert-body { \
                color: #cf222e; \
            } \
            label.alert-header { \
                font-size: 14px; \
            } \
            label.alert-body { \
                font-size: 13px; \
            }",
        );
        gtk::style_context_add_provider_for_display(
            &Display::default().expect("Could not get default display"),
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
