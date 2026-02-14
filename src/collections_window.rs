use std::cell::RefCell;
use std::rc::Rc;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{ApplicationWindow, Application, Button, Box, Label, ListBox, ListBoxRow, ScrolledWindow};
use crate::wallpaper_manager::WallpaperManager;

pub fn open_collections_window(
    app: &Application,
    parent: &ApplicationWindow,
    manager: Rc<RefCell<WallpaperManager>>,
    on_profile_applied: impl Fn(&str) + 'static,
) {
    let on_profile_applied = Rc::new(on_profile_applied);

    let window = ApplicationWindow::builder()
        .application(app)
        .transient_for(parent)
        .default_width(700)
        .default_height(500)
        .title("Collections")
        .build();

    let root_box = Box::new(gtk::Orientation::Vertical, 8);
    root_box.set_margin_top(8);
    root_box.set_margin_bottom(8);
    root_box.set_margin_start(8);
    root_box.set_margin_end(8);

    // --- Top bar ---
    let top_bar = Box::new(gtk::Orientation::Horizontal, 6);
    let title_label = Label::new(Some("Collections"));
    title_label.add_css_class("title-3");
    title_label.set_hexpand(true);
    title_label.set_xalign(0.0);
    let new_collection_btn = Button::with_label("New");
    let delete_collection_btn = Button::with_label("Delete");
    top_bar.append(&title_label);
    top_bar.append(&new_collection_btn);
    top_bar.append(&delete_collection_btn);
    root_box.append(&top_bar);

    // --- Content: left + right panels ---
    let content = Box::new(gtk::Orientation::Horizontal, 8);
    content.set_vexpand(true);

    // Left panel: collection list
    let left_panel = Box::new(gtk::Orientation::Vertical, 4);
    left_panel.set_width_request(200);
    let left_scroll = ScrolledWindow::new();
    left_scroll.set_vexpand(true);
    let collection_listbox = ListBox::new();
    collection_listbox.set_selection_mode(gtk::SelectionMode::Single);
    left_scroll.set_child(Some(&collection_listbox));
    left_panel.append(&left_scroll);

    // Right panel: profiles in selected collection
    let right_panel = Box::new(gtk::Orientation::Vertical, 4);
    right_panel.set_hexpand(true);

    let right_header = Box::new(gtk::Orientation::Horizontal, 6);
    let collection_name_label = Label::new(Some("(select a collection)"));
    collection_name_label.set_hexpand(true);
    collection_name_label.set_xalign(0.0);
    let add_profile_btn = Button::with_label("Add Profile");
    let remove_profile_btn = Button::with_label("Remove");
    add_profile_btn.set_sensitive(false);
    remove_profile_btn.set_sensitive(false);
    right_header.append(&collection_name_label);
    right_header.append(&add_profile_btn);
    right_header.append(&remove_profile_btn);
    right_panel.append(&right_header);

    let right_scroll = ScrolledWindow::new();
    right_scroll.set_vexpand(true);
    let profile_listbox = ListBox::new();
    profile_listbox.set_selection_mode(gtk::SelectionMode::Single);
    right_scroll.set_child(Some(&profile_listbox));
    right_panel.append(&right_scroll);

    // Action bar: Random / Prev / Next
    let action_bar = Box::new(gtk::Orientation::Horizontal, 6);
    let random_btn = Button::with_label("Random");
    let prev_btn = Button::with_label("Prev");
    let next_btn = Button::with_label("Next");
    random_btn.set_sensitive(false);
    prev_btn.set_sensitive(false);
    next_btn.set_sensitive(false);
    action_bar.append(&random_btn);
    action_bar.append(&prev_btn);
    action_bar.append(&next_btn);
    right_panel.append(&action_bar);

    let status_label = Label::new(None);
    status_label.set_xalign(0.0);
    right_panel.append(&status_label);

    content.append(&left_panel);
    content.append(&right_panel);
    root_box.append(&content);

    window.set_child(Some(&root_box));

    // --- Shared state ---
    let selected_collection: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // Helper: populate left listbox from manager
    let populate_collections = {
        let collection_listbox = collection_listbox.clone();
        let manager = manager.clone();
        Rc::new(move || {
            // Remove all existing rows
            while let Some(child) = collection_listbox.first_child() {
                collection_listbox.remove(&child);
            }
            let mgr = manager.borrow();
            let mut names: Vec<&String> = mgr.collections.keys().collect();
            names.sort();
            for name in names {
                let row = ListBoxRow::new();
                let label = Label::new(Some(name));
                label.set_xalign(0.0);
                label.set_margin_start(4);
                label.set_margin_end(4);
                label.set_margin_top(2);
                label.set_margin_bottom(2);
                row.set_child(Some(&label));
                collection_listbox.append(&row);
            }
        })
    };

    // Helper: populate right listbox with profiles of selected collection
    let populate_profiles = {
        let profile_listbox = profile_listbox.clone();
        let manager = manager.clone();
        let collection_name_label = collection_name_label.clone();
        let selected_collection = selected_collection.clone();
        let add_profile_btn = add_profile_btn.clone();
        let remove_profile_btn = remove_profile_btn.clone();
        let random_btn = random_btn.clone();
        let prev_btn = prev_btn.clone();
        let next_btn = next_btn.clone();
        Rc::new(move || {
            while let Some(child) = profile_listbox.first_child() {
                profile_listbox.remove(&child);
            }
            let sel = selected_collection.borrow();
            if let Some(ref col_name) = *sel {
                collection_name_label.set_text(col_name);
                add_profile_btn.set_sensitive(true);
                remove_profile_btn.set_sensitive(true);

                let mgr = manager.borrow();
                let valid = mgr.get_valid_collection_profiles(col_name);
                let has_profiles = !valid.is_empty();
                random_btn.set_sensitive(has_profiles);
                prev_btn.set_sensitive(has_profiles);
                next_btn.set_sensitive(has_profiles);

                // Show all profiles in the collection (including invalid ones, marked)
                if let Some(col) = mgr.collections.get(col_name) {
                    for profile_name in &col.profiles {
                        let row = ListBoxRow::new();
                        let display = if mgr.profiles.contains_key(profile_name) {
                            profile_name.clone()
                        } else {
                            format!("{} (missing)", profile_name)
                        };
                        let label = Label::new(Some(&display));
                        label.set_xalign(0.0);
                        label.set_margin_start(4);
                        label.set_margin_end(4);
                        label.set_margin_top(2);
                        label.set_margin_bottom(2);
                        row.set_child(Some(&label));
                        profile_listbox.append(&row);
                    }
                }
            } else {
                collection_name_label.set_text("(select a collection)");
                add_profile_btn.set_sensitive(false);
                remove_profile_btn.set_sensitive(false);
                random_btn.set_sensitive(false);
                prev_btn.set_sensitive(false);
                next_btn.set_sensitive(false);
            }
        })
    };

    // Initial population
    populate_collections();

    // --- Collection selection ---
    collection_listbox.connect_row_selected({
        let selected_collection = selected_collection.clone();
        let populate_profiles = populate_profiles.clone();
        let status_label = status_label.clone();
        move |_, row| {
            if let Some(row) = row {
                if let Some(label) = row.child().and_then(|c| c.downcast::<Label>().ok()) {
                    *selected_collection.borrow_mut() = Some(label.text().to_string());
                }
            } else {
                *selected_collection.borrow_mut() = None;
            }
            status_label.set_text("");
            populate_profiles();
        }
    });

    // --- New Collection button ---
    new_collection_btn.connect_clicked({
        let window = window.clone();
        let manager = manager.clone();
        let populate_collections = populate_collections.clone();
        move |_| {
            let dialog = gtk::Dialog::builder()
                .transient_for(&window)
                .modal(true)
                .title("New Collection")
                .build();
            dialog.add_button("Cancel", gtk::ResponseType::Cancel);
            dialog.add_button("Create", gtk::ResponseType::Ok);

            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("Enter collection name"));
            let content_area = dialog.content_area();
            content_area.append(&Label::new(Some("Collection Name:")));
            content_area.append(&entry);
            dialog.show();

            let manager = manager.clone();
            let populate_collections = populate_collections.clone();
            dialog.connect_response(move |d, resp| {
                if resp == gtk::ResponseType::Ok {
                    let name = entry.text().to_string();
                    if !name.trim().is_empty() {
                        let mut mgr = manager.borrow_mut();
                        mgr.create_collection(name.trim());
                        mgr.save_config("config.json");
                        drop(mgr);
                        populate_collections();
                    }
                }
                d.close();
            });
        }
    });

    // --- Delete Collection button ---
    delete_collection_btn.connect_clicked({
        let manager = manager.clone();
        let selected_collection = selected_collection.clone();
        let populate_collections = populate_collections.clone();
        let populate_profiles = populate_profiles.clone();
        let collection_listbox = collection_listbox.clone();
        move |_| {
            let sel = selected_collection.borrow().clone();
            if let Some(col_name) = sel {
                let mut mgr = manager.borrow_mut();
                mgr.delete_collection(&col_name);
                mgr.save_config("config.json");
                drop(mgr);
                *selected_collection.borrow_mut() = None;
                collection_listbox.unselect_all();
                populate_collections();
                populate_profiles();
            }
        }
    });

    // --- Add Profile button ---
    add_profile_btn.connect_clicked({
        let window = window.clone();
        let manager = manager.clone();
        let selected_collection = selected_collection.clone();
        let populate_profiles = populate_profiles.clone();
        move |_| {
            let sel = selected_collection.borrow().clone();
            let col_name = match sel {
                Some(name) => name,
                None => return,
            };

            // Get profiles not already in this collection
            let mgr = manager.borrow();
            let existing: Vec<String> = mgr.collections.get(&col_name)
                .map(|c| c.profiles.clone())
                .unwrap_or_default();
            let mut available: Vec<String> = mgr.profiles.keys()
                .filter(|p| !existing.contains(p))
                .cloned()
                .collect();
            available.sort();
            drop(mgr);

            if available.is_empty() {
                return;
            }

            let dialog = gtk::Dialog::builder()
                .transient_for(&window)
                .modal(true)
                .title("Add Profile to Collection")
                .build();
            dialog.add_button("Cancel", gtk::ResponseType::Cancel);
            dialog.add_button("Add", gtk::ResponseType::Ok);

            let strs: Vec<&str> = available.iter().map(|s| s.as_str()).collect();
            let dropdown = gtk::DropDown::from_strings(&strs);
            let content_area = dialog.content_area();
            content_area.append(&Label::new(Some("Select profile:")));
            content_area.append(&dropdown);
            dialog.show();

            let manager = manager.clone();
            let populate_profiles = populate_profiles.clone();
            dialog.connect_response(move |d, resp| {
                if resp == gtk::ResponseType::Ok {
                    if let Some(item) = dropdown.selected_item() {
                        if let Ok(so) = item.downcast::<gtk::StringObject>() {
                            let profile_name = so.string().to_string();
                            let mut mgr = manager.borrow_mut();
                            mgr.add_profile_to_collection(&col_name, &profile_name);
                            mgr.save_config("config.json");
                            drop(mgr);
                            populate_profiles();
                        }
                    }
                }
                d.close();
            });
        }
    });

    // --- Remove Profile button ---
    remove_profile_btn.connect_clicked({
        let manager = manager.clone();
        let selected_collection = selected_collection.clone();
        let profile_listbox = profile_listbox.clone();
        let populate_profiles = populate_profiles.clone();
        move |_| {
            let sel = selected_collection.borrow().clone();
            let col_name = match sel {
                Some(name) => name,
                None => return,
            };
            if let Some(row) = profile_listbox.selected_row() {
                // Get the profile name from the row's label
                if let Some(label) = row.child().and_then(|c| c.downcast::<Label>().ok()) {
                    let text = label.text().to_string();
                    // Strip " (missing)" suffix if present
                    let profile_name = text.trim_end_matches(" (missing)");
                    let mut mgr = manager.borrow_mut();
                    mgr.remove_profile_from_collection(&col_name, profile_name);
                    mgr.save_config("config.json");
                    drop(mgr);
                    populate_profiles();
                }
            }
        }
    });

    // --- Random button ---
    random_btn.connect_clicked({
        let manager = manager.clone();
        let selected_collection = selected_collection.clone();
        let status_label = status_label.clone();
        let on_profile_applied = on_profile_applied.clone();
        move |_| {
            let sel = selected_collection.borrow().clone();
            if let Some(col_name) = sel {
                let result = manager.borrow_mut().apply_random_from_collection(&col_name);
                if let Some(name) = result {
                    status_label.set_text(&format!("Applied: {}", name));
                    on_profile_applied(&name);
                }
            }
        }
    });

    // --- Prev button ---
    prev_btn.connect_clicked({
        let manager = manager.clone();
        let selected_collection = selected_collection.clone();
        let status_label = status_label.clone();
        let on_profile_applied = on_profile_applied.clone();
        move |_| {
            let sel = selected_collection.borrow().clone();
            if let Some(col_name) = sel {
                let result = manager.borrow_mut().apply_prev_in_collection(&col_name);
                if let Some(name) = result {
                    status_label.set_text(&format!("Applied: {}", name));
                    on_profile_applied(&name);
                }
            }
        }
    });

    // --- Next button ---
    next_btn.connect_clicked({
        let manager = manager.clone();
        let selected_collection = selected_collection.clone();
        let status_label = status_label.clone();
        let on_profile_applied = on_profile_applied.clone();
        move |_| {
            let sel = selected_collection.borrow().clone();
            if let Some(col_name) = sel {
                let result = manager.borrow_mut().apply_next_in_collection(&col_name);
                if let Some(name) = result {
                    status_label.set_text(&format!("Applied: {}", name));
                    on_profile_applied(&name);
                }
            }
        }
    });

    window.present();
}
