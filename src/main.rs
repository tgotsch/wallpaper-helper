use std::cell::RefCell;
use std::rc::Rc;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{ApplicationWindow, Application, Button, Box, Picture, FileChooserDialog};
use gtk::glib;
use crate::alert_box::{AlertBox, AlertLevel};
use crate::wallpaper_manager::WallpaperManager;

mod alert_box;
mod backend;
mod collections_window;
mod tray;
mod wallpaper_manager;
//mod wallpaper_source;

#[derive(Clone)]
struct WallpaperData {
    monitor_name: String,
    filename:  String,
    image: Picture,
    button: Button,
}

impl WallpaperData {
    fn new(monitor_name: String, initial_filename: String) -> Self {
        let image = Picture::new();
        let button = Button::new();

        // Set initial image if file exists
        if !initial_filename.is_empty() {
            image.set_filename(Some(&initial_filename));
        }

        image.set_can_shrink(true);
        image.set_hexpand(true);
        image.set_vexpand(true);

        // Add the image to the button
        button.set_child(Some(&image));

        // Make the button look like just an image (remove default styling)
        button.add_css_class("flat");
        button.set_has_frame(false);
        button.set_hexpand(true);
        button.set_vexpand(true);

        Self {
            monitor_name,
            filename: initial_filename,
            image,
            button,
        }
    }

    fn setup_click_handler(&self, wallpapers: Rc<RefCell<Vec<WallpaperData>>>) {
        let monitor_name = self.monitor_name.clone();
        let image = self.image.clone();
        let wallpapers_clone = wallpapers.clone();

        self.button.connect_clicked(move |button| {
            let window = button.root()
                .and_then(|root| root.downcast::<gtk::Window>().ok());

            let dialog = FileChooserDialog::new(
                Some("Select Wallpaper"),
                window.as_ref(),
                gtk::FileChooserAction::Open,
                &[
                    ("_Cancel", gtk::ResponseType::Cancel),
                    ("_Open", gtk::ResponseType::Accept),
                ],
            );

            // Set up file filters for images
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Image files"));
            filter.add_mime_type("image/*");
            filter.add_pattern("*.jpg");
            filter.add_pattern("*.jpeg");
            filter.add_pattern("*.png");
            filter.add_pattern("*.gif");
            filter.add_pattern("*.bmp");
            filter.add_pattern("*.webp");

            dialog.set_filter(&filter);

            let monitor_name_clone = monitor_name.clone();
            let image_clone = image.clone();
            let wallpapers_clone2 = wallpapers_clone.clone();

            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            let path_str = path.to_string_lossy().to_string();

                            // Update the image
                            image_clone.set_filename(Some(&path_str));

                            // Update the data structure
                            if let Ok(mut wallpapers_borrowed) = wallpapers_clone2.try_borrow_mut() {
                                if let Some(wallpaper_data) = wallpapers_borrowed
                                    .iter_mut()
                                    .find(|w| w.monitor_name == monitor_name_clone)
                                {
                                    wallpaper_data.filename = path_str;
                                }
                            }
                        }
                    }
                }
                dialog.close();
            });

            dialog.show();
        });
    }
}

#[derive(Clone, Debug, PartialEq)]
enum DropdownEntry {
    Profile(String),
    Collection(String),
}

impl DropdownEntry {
    fn display_label(&self) -> String {
        match self {
            DropdownEntry::Profile(name) => name.clone(),
            DropdownEntry::Collection(name) => format!("[Collection] {}", name),
        }
    }

}

fn build_ui(app: &gtk::Application, window_cell: &Rc<RefCell<Option<ApplicationWindow>>>, config_path: &str) {
    let manager = Rc::new(RefCell::new(WallpaperManager::new()));
    let wallpapers: Rc<RefCell<Vec<WallpaperData>>> = Rc::new(RefCell::new(Vec::new()));
    manager.borrow_mut().load_config(config_path);

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1000)
        .default_height(1000)
        .title("Wallpaper Helper")
        .build();

    let v_box = Box::new(gtk::Orientation::Vertical, 10);
    let grid = gtk::Grid::new();
    grid.set_column_spacing(5);

    let alias_monitors = manager.borrow().get_alias_monitor_info();
    for (i, (alias, monitor_info)) in alias_monitors.iter().enumerate() {
        let wallpaper_str = manager.borrow().get_current_wallpaper_by_alias(alias);

        let wallpaper_data = WallpaperData::new(alias.clone(), wallpaper_str.to_string());
        wallpaper_data.setup_click_handler(wallpapers.clone());

        wallpapers.borrow_mut().push(wallpaper_data.clone());

        let label_text = match monitor_info {
            Some(info) => format!("{} ({}x{})", alias, info.width, info.height),
            None => format!("{} (not connected)", alias),
        };
        let label = gtk::Label::new(Some(&label_text));

        grid.attach(&label, i as i32, 0, 1, 1);
        grid.attach(&wallpaper_data.button, i as i32, 1, 1, 1);
    }

    // Create dropdown with empty model (populated by rebuild_dropdown below)
    let profile_selector = gtk::DropDown::from_strings(&[] as &[&str]);

    AlertBox::load_css();
    let warning_alert = AlertBox::new(AlertLevel::Warning);

    let apply_button = Button::with_label("Apply Selected profile");

    // --- Collection controls (initially hidden) ---
    let collection_controls = Box::new(gtk::Orientation::Vertical, 6);
    collection_controls.set_visible(false);

    let collection_action_bar = Box::new(gtk::Orientation::Horizontal, 6);
    let prev_btn = Button::with_label("Prev");
    let random_btn = Button::with_label("Random");
    let next_btn = Button::with_label("Next");
    collection_action_bar.append(&prev_btn);
    collection_action_bar.append(&random_btn);
    collection_action_bar.append(&next_btn);
    collection_controls.append(&collection_action_bar);

    let slideshow_bar = Box::new(gtk::Orientation::Horizontal, 6);
    let slideshow_label = gtk::Label::new(Some("Slideshow:"));
    let slideshow_spin = gtk::SpinButton::with_range(1.0, 120.0, 1.0);
    slideshow_spin.set_value(15.0);
    slideshow_spin.set_digits(0);
    let slideshow_min_label = gtk::Label::new(Some("min"));
    let slideshow_toggle = Button::with_label("Start");
    slideshow_bar.append(&slideshow_label);
    slideshow_bar.append(&slideshow_spin);
    slideshow_bar.append(&slideshow_min_label);
    slideshow_bar.append(&slideshow_toggle);
    collection_controls.append(&slideshow_bar);

    let collection_status = gtk::Label::new(None);
    collection_status.set_xalign(0.0);
    collection_controls.append(&collection_status);

    // --- Shared state ---
    let dropdown_entries: Rc<RefCell<Vec<DropdownEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let suppressing_signal: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let slideshow_source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    // Helper: get currently selected dropdown entry
    let get_selected_entry = {
        let dropdown = profile_selector.clone();
        let dropdown_entries = dropdown_entries.clone();
        Rc::new(move || -> Option<DropdownEntry> {
            let idx = dropdown.selected() as usize;
            let entries = dropdown_entries.borrow();
            entries.get(idx).cloned()
        })
    };

    // Helper: stop slideshow
    let stop_slideshow = {
        let slideshow_source = slideshow_source.clone();
        let slideshow_toggle = slideshow_toggle.clone();
        let slideshow_spin = slideshow_spin.clone();
        Rc::new(move || {
            if let Some(source_id) = slideshow_source.borrow_mut().take() {
                source_id.remove();
            }
            slideshow_toggle.set_label("Start");
            slideshow_spin.set_sensitive(true);
        })
    };

    // Helper: update UI for the current dropdown selection
    let update_ui_for_selection = {
        let get_selected_entry = get_selected_entry.clone();
        let wallpapers = wallpapers.clone();
        let manager = manager.clone();
        let warning_alert = warning_alert.clone();
        let apply_button = apply_button.clone();
        let collection_controls = collection_controls.clone();
        let collection_status = collection_status.clone();
        Rc::new(move || {
            let entry = get_selected_entry();
            match entry {
                Some(DropdownEntry::Profile(ref name)) => {
                    apply_button.set_visible(true);
                    collection_controls.set_visible(false);
                    for w in wallpapers.borrow().iter() {
                        w.button.set_sensitive(true);
                    }
                    update_wallpaper_images_for_profile(name, &wallpapers.borrow(), &manager);
                    update_warning_alert_for_profile(name, &warning_alert, &manager);
                }
                Some(DropdownEntry::Collection(ref col_name)) => {
                    apply_button.set_visible(false);
                    collection_controls.set_visible(true);
                    for w in wallpapers.borrow().iter() {
                        w.button.set_sensitive(false);
                    }
                    let mgr = manager.borrow();
                    let valid = mgr.get_valid_collection_profiles(col_name);
                    if valid.is_empty() {
                        for w in wallpapers.borrow().iter() {
                            w.image.set_filename(None::<&str>);
                        }
                        warning_alert.show("Collection has no valid profiles");
                        collection_status.set_text("");
                    } else {
                        let idx = mgr.collection_cycle_indices.get(col_name).copied().unwrap_or(0);
                        let clamped_idx = idx.min(valid.len() - 1);
                        let current_profile = valid[clamped_idx].clone();
                        drop(mgr);
                        update_wallpaper_images_for_profile(&current_profile, &wallpapers.borrow(), &manager);
                        update_warning_alert_for_profile(&current_profile, &warning_alert, &manager);
                        collection_status.set_text(&format!("Current: {}", current_profile));
                    }
                }
                None => {
                    apply_button.set_visible(false);
                    collection_controls.set_visible(false);
                    warning_alert.hide();
                }
            }
        })
    };

    // Helper: rebuild dropdown from manager state
    let rebuild_dropdown = {
        let dropdown = profile_selector.clone();
        let dropdown_entries = dropdown_entries.clone();
        let manager = manager.clone();
        let suppressing_signal = suppressing_signal.clone();
        let update_ui = update_ui_for_selection.clone();
        Rc::new(move |preserve: Option<DropdownEntry>| {
            *suppressing_signal.borrow_mut() = true;

            let mgr = manager.borrow();
            let mut profile_names: Vec<String> = mgr.profiles.keys().cloned().collect();
            profile_names.sort();
            let mut collection_names: Vec<String> = mgr.collections.keys().cloned().collect();
            collection_names.sort();
            drop(mgr);

            let mut entries: Vec<DropdownEntry> = Vec::new();
            for name in &profile_names {
                entries.push(DropdownEntry::Profile(name.clone()));
            }
            for name in &collection_names {
                entries.push(DropdownEntry::Collection(name.clone()));
            }

            let labels: Vec<String> = entries.iter().map(|e| e.display_label()).collect();
            let label_strs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();

            if let Some(model) = dropdown.model() {
                if let Ok(string_list) = model.downcast::<gtk::StringList>() {
                    let n = string_list.n_items();
                    string_list.splice(0, n, &label_strs);
                }
            }

            let select_idx = if let Some(ref target) = preserve {
                entries.iter().position(|e| e == target).unwrap_or(0)
            } else {
                entries.iter()
                    .position(|e| matches!(e, DropdownEntry::Profile(n) if n == "default"))
                    .unwrap_or(0)
            };

            *dropdown_entries.borrow_mut() = entries;
            dropdown.set_selected(select_idx as u32);
            *suppressing_signal.borrow_mut() = false;

            update_ui();
        })
    };

    // --- Dropdown selection changed ---
    profile_selector.connect_selected_notify({
        let suppressing_signal = suppressing_signal.clone();
        let stop_slideshow = stop_slideshow.clone();
        let update_ui = update_ui_for_selection.clone();
        let collection_status = collection_status.clone();
        move |_| {
            if *suppressing_signal.borrow() {
                return;
            }
            stop_slideshow();
            collection_status.set_text("");
            update_ui();
        }
    });

    // Initial dropdown population
    rebuild_dropdown(None);

    // --- Apply button ---
    apply_button.connect_clicked({
        let get_selected_entry = get_selected_entry.clone();
        let manager = manager.clone();
        move |_| {
            if let Some(DropdownEntry::Profile(ref name)) = get_selected_entry() {
                println!("Applying profile: '{}'", name);
                manager.borrow_mut().apply_profile(name);
            }
        }
    });

    // --- New profile button ---
    let new_button = Button::with_label("New profile");
    let config_path: Rc<str> = Rc::from(config_path);
    new_button.connect_clicked({
        let window = window.clone();
        let manager = manager.clone();
        let wallpapers = wallpapers.clone();
        let rebuild_dropdown = rebuild_dropdown.clone();
        let config_path = config_path.clone();
        move |_| {
            let dialog = gtk::Dialog::builder()
                .transient_for(&window)
                .modal(true)
                .title("Create New Profile")
                .build();

            dialog.add_button("Cancel", gtk::ResponseType::Cancel);
            dialog.add_button("Create", gtk::ResponseType::Ok);

            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("Enter profile name"));
            let content_area = dialog.content_area();
            content_area.append(&gtk::Label::new(Some("Profile Name:")));
            content_area.append(&entry);
            dialog.show();

            let manager = manager.clone();
            let wallpapers = wallpapers.clone();
            let rebuild_dropdown = rebuild_dropdown.clone();
            let config_path = config_path.clone();
            dialog.connect_response(move |d, resp| {
                if resp == gtk::ResponseType::Ok {
                    let name = entry.text().to_string();
                    if !name.trim().is_empty() {
                        println!("Creating profile: {}", name);
                        manager.borrow_mut().create_profile(name.as_str());
                        for wallpaper in wallpapers.borrow().iter() {
                            manager.borrow_mut().set_wallpaper_in_profile(
                                name.as_str(),
                                wallpaper.monitor_name.as_str(),
                                wallpaper.filename.as_str(),
                            );
                        }
                        manager.borrow_mut().save_config(&config_path);
                        rebuild_dropdown(Some(DropdownEntry::Profile(name)));
                    }
                }
                d.close();
            });
        }
    });

    // --- Collections button ---
    let collections_button = Button::with_label("Collections...");
    collections_button.connect_clicked({
        let app = app.clone();
        let window = window.clone();
        let manager = manager.clone();
        let get_selected_entry = get_selected_entry.clone();
        let rebuild_dropdown = rebuild_dropdown.clone();
        move |_| {
            let get_selected_entry = get_selected_entry.clone();
            let rebuild_dropdown = rebuild_dropdown.clone();
            collections_window::open_collections_window(
                &app,
                &window,
                manager.clone(),
                move || {
                    let current = get_selected_entry();
                    rebuild_dropdown(current);
                },
            );
        }
    });

    // --- Random button ---
    random_btn.connect_clicked({
        let get_selected_entry = get_selected_entry.clone();
        let manager = manager.clone();
        let wallpapers = wallpapers.clone();
        let warning_alert = warning_alert.clone();
        let collection_status = collection_status.clone();
        move |_| {
            if let Some(DropdownEntry::Collection(ref col_name)) = get_selected_entry() {
                let result = manager.borrow_mut().apply_random_from_collection(col_name);
                if let Some(name) = result {
                    update_wallpaper_images_for_profile(&name, &wallpapers.borrow(), &manager);
                    update_warning_alert_for_profile(&name, &warning_alert, &manager);
                    collection_status.set_text(&format!("Applied: {}", name));
                }
            }
        }
    });

    // --- Prev button ---
    prev_btn.connect_clicked({
        let get_selected_entry = get_selected_entry.clone();
        let manager = manager.clone();
        let wallpapers = wallpapers.clone();
        let warning_alert = warning_alert.clone();
        let collection_status = collection_status.clone();
        move |_| {
            if let Some(DropdownEntry::Collection(ref col_name)) = get_selected_entry() {
                let result = manager.borrow_mut().apply_prev_in_collection(col_name);
                if let Some(name) = result {
                    update_wallpaper_images_for_profile(&name, &wallpapers.borrow(), &manager);
                    update_warning_alert_for_profile(&name, &warning_alert, &manager);
                    collection_status.set_text(&format!("Applied: {}", name));
                }
            }
        }
    });

    // --- Next button ---
    next_btn.connect_clicked({
        let get_selected_entry = get_selected_entry.clone();
        let manager = manager.clone();
        let wallpapers = wallpapers.clone();
        let warning_alert = warning_alert.clone();
        let collection_status = collection_status.clone();
        move |_| {
            if let Some(DropdownEntry::Collection(ref col_name)) = get_selected_entry() {
                let result = manager.borrow_mut().apply_next_in_collection(col_name);
                if let Some(name) = result {
                    update_wallpaper_images_for_profile(&name, &wallpapers.borrow(), &manager);
                    update_warning_alert_for_profile(&name, &warning_alert, &manager);
                    collection_status.set_text(&format!("Applied: {}", name));
                }
            }
        }
    });

    // --- Slideshow toggle ---
    slideshow_toggle.connect_clicked({
        let get_selected_entry = get_selected_entry.clone();
        let manager = manager.clone();
        let wallpapers = wallpapers.clone();
        let warning_alert = warning_alert.clone();
        let collection_status = collection_status.clone();
        let slideshow_source = slideshow_source.clone();
        let slideshow_spin = slideshow_spin.clone();
        let slideshow_toggle = slideshow_toggle.clone();
        let stop_slideshow = stop_slideshow.clone();
        move |_| {
            if slideshow_source.borrow().is_some() {
                stop_slideshow();
                return;
            }

            let col_name = match get_selected_entry() {
                Some(DropdownEntry::Collection(name)) => name,
                _ => return,
            };

            let interval_min = slideshow_spin.value() as u32;
            let interval_sec = interval_min * 60;

            slideshow_toggle.set_label("Stop");
            slideshow_spin.set_sensitive(false);

            let source_id = glib::timeout_add_seconds_local(interval_sec, {
                let manager = manager.clone();
                let wallpapers = wallpapers.clone();
                let warning_alert = warning_alert.clone();
                let collection_status = collection_status.clone();
                let slideshow_source = slideshow_source.clone();
                let slideshow_toggle = slideshow_toggle.clone();
                let slideshow_spin = slideshow_spin.clone();
                let col_name = col_name.clone();
                move || {
                    let result = manager.borrow_mut().apply_next_in_collection(&col_name);
                    if let Some(name) = result {
                        update_wallpaper_images_for_profile(&name, &wallpapers.borrow(), &manager);
                        update_warning_alert_for_profile(&name, &warning_alert, &manager);
                        collection_status.set_text(&format!("Applied: {}", name));
                        glib::ControlFlow::Continue
                    } else {
                        if let Some(sid) = slideshow_source.borrow_mut().take() {
                            sid.remove();
                        }
                        slideshow_toggle.set_label("Start");
                        slideshow_spin.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                }
            });
            *slideshow_source.borrow_mut() = Some(source_id);
        }
    });

    // --- Layout ---
    let selector_and_new_profile = Box::new(gtk::Orientation::Horizontal, 5);
    selector_and_new_profile.append(&profile_selector);
    selector_and_new_profile.append(&new_button);
    selector_and_new_profile.append(&collections_button);

    v_box.append(&selector_and_new_profile);
    v_box.append(&warning_alert.container);
    v_box.append(&grid);
    v_box.append(&apply_button);
    v_box.append(&collection_controls);
    window.set_child(Some(&v_box));

    // Close-to-hide: hide window instead of quitting
    window.connect_close_request({
        let app = app.clone();
        move |w| {
            w.set_visible(false);
            let notification = gtk::gio::Notification::new("Wallpaper Helper");
            notification.set_body(Some("Still running in the system tray."));
            app.send_notification(Some("minimized-to-tray"), &notification);
            glib::Propagation::Stop
        }
    });

    // Keep app alive when all windows are hidden
    let _hold_guard = app.hold();

    // Store window reference for re-activation
    *window_cell.borrow_mut() = Some(window.clone());

    // --- System tray ---
    let tray = tray::AppTray::new();

    glib::timeout_add_local(std::time::Duration::from_millis(100), {
        let window = window.clone();
        let app = app.clone();
        move || {
            let _keep_alive = &tray;
            for action in tray.poll_actions() {
                match action {
                    tray::TrayAction::ShowWindow => {
                        window.set_visible(true);
                        window.present();
                    }
                    tray::TrayAction::Quit => {
                        app.quit();
                    }
                }
            }
            glib::ControlFlow::Continue
        }
    });

    // --- Scheduler timer ---
    glib::timeout_add_seconds_local(30, {
        let manager = manager.clone();
        move || {
            if let Some(name) = manager.borrow_mut().check_and_apply_schedule() {
                println!("Scheduler applied profile: {}", name);
            }
            glib::ControlFlow::Continue
        }
    });

    window.present();
}

fn update_warning_alert_for_profile(profile_name: &str, alert: &AlertBox, manager: &Rc<RefCell<WallpaperManager>>) {
    let mgr = manager.borrow();
    let mut parts = Vec::new();

    if let Some(info) = mgr.check_profile_mismatch(profile_name) {
        if !info.extra_aliases.is_empty() {
            parts.push(format!("Profile has unknown aliases: {}", info.extra_aliases.join(", ")));
        }
        if !info.missing_aliases.is_empty() {
            parts.push(format!("No wallpaper set for: {}", info.missing_aliases.join(", ")));
        }
    }

    if let Some(profile) = mgr.profiles.get(profile_name) {
        let mut missing_files = Vec::new();
        for (alias, relative_path) in &profile.monitor_wallpapers {
            if relative_path.is_empty() {
                continue;
            }
            let abs = mgr.resolve_wallpaper_path(relative_path);
            if !std::path::Path::new(&abs).exists() {
                missing_files.push(format!("{} ({})", alias, relative_path));
            }
        }
        missing_files.sort();
        if !missing_files.is_empty() {
            parts.push(format!("Image not found for: {}", missing_files.join(", ")));
        }
    }

    if !parts.is_empty() {
        alert.show(&parts.join("\n"));
    } else {
        alert.hide();
    }
}

fn update_wallpaper_images_for_profile(profile_name: &str,
                                       wallpapers: &Vec<WallpaperData>,
                                       manager: &Rc<RefCell<WallpaperManager>>) {
    let mgr = manager.borrow();
    let profile = mgr.profiles.get(profile_name);

    for wallpaper in wallpapers.iter() {
        let path = profile
            .and_then(|p| p.monitor_wallpapers.get(&wallpaper.monitor_name))
            .map(|rel| mgr.resolve_wallpaper_path(rel));

        match path {
            Some(abs) if !abs.is_empty() && std::path::Path::new(&abs).exists() => {
                wallpaper.image.set_filename(Some(abs.as_str()));
            }
            _ => {
                wallpaper.image.set_filename(None::<&str>);
            }
        }
    }
}

fn main() {
    let config_path = std::env::args().nth(1).unwrap_or_else(|| "config.json".to_string());

    let app = Application::builder()
        .application_id("com.wallpaperhelper.app")
        .build();

    let window_cell: Rc<RefCell<Option<ApplicationWindow>>> = Rc::new(RefCell::new(None));

    app.connect_activate({
        let window_cell = window_cell.clone();
        move |app| {
            if let Some(ref window) = *window_cell.borrow() {
                window.set_visible(true);
                window.present();
                return;
            }
            build_ui(app, &window_cell, &config_path);
        }
    });

    app.run_with_args(&[] as &[&str]);
}