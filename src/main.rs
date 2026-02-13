use std::cell::RefCell;
use std::rc::Rc;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{ApplicationWindow, Application, Button, Box, Picture, FileChooserDialog};
use gtk4::DropDown;
use crate::alert_box::{AlertBox, AlertLevel};
use crate::wallpaper_manager::WallpaperManager;

mod alert_box;
mod backend;
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

fn build_ui(app: &gtk::Application, config_path: &str) {
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
    for (i, (alias, monitor_info)) in alias_monitors.iter().enumerate()
    {
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

    let mut profile_names: Vec<String> = manager.borrow().profiles.keys().cloned().collect();
    profile_names.sort();
    let profile_name_strs: Vec<&str> = profile_names.iter().map(|s| s.as_str()).collect();
    let profile_selector = gtk::DropDown::from_strings(profile_name_strs.as_slice());

    // Select "default" profile if it exists, otherwise first
    let initial_index = profile_names.iter().position(|n| n == "default").unwrap_or(0);
    profile_selector.set_selected(initial_index as u32);

    AlertBox::load_css();
    let warning_alert = AlertBox::new(AlertLevel::Warning);

    profile_selector.connect_selected_notify({
        let dropdown = profile_selector.clone();
        let wallpapers_cloned = wallpapers.clone();
        let manager_clone = manager.clone();
        let warning_clone = warning_alert.clone();
        move |_| {
            update_wallpaper_images_from_profile(&dropdown, &wallpapers_cloned.borrow(), &manager_clone);
            update_warning_alert(&dropdown, &warning_clone, &manager_clone);
        }
    });

    update_wallpaper_images_from_profile(&profile_selector, &wallpapers.borrow(), &manager);
    update_warning_alert(&profile_selector, &warning_alert, &manager);

    let apply_button = Button::with_label("Apply Selected profile");
    apply_button.connect_clicked({
        let dropdown = profile_selector.clone();
        let manager_clone = manager.clone();
        move |_|
            {
                println!("Apply button clicked");
                if let Some(selected_item) = dropdown.selected_item()
                {
                    if let Ok(string_object) = selected_item.downcast::<gtk::StringObject>()
                    {
                        let selected_text = string_object.string();
                        println!("Applying profile: '{}'", selected_text);
                        manager_clone.borrow_mut().apply_profile(&selected_text);
                    }
                } else {
                    println!("No profile selected in dropdown");
                }
            }
    });

    let new_button = Button::with_label("New profile");
    let parent_clone = window.clone();
    let config_path: Rc<str> = Rc::from(config_path);
    new_button.connect_clicked(move |_| {
        // Create the dialog, with OK and Cancel buttons
        let dialog = gtk::Dialog::builder()
            .transient_for(&parent_clone)
            .modal(true)
            .title("Create New Profile")
            .build();

        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button("Create", gtk::ResponseType::Ok);

        // Entry for profile name
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("Enter profile name"));

        // Add to dialog content area
        let content_area = dialog.content_area();
        content_area.append(&gtk::Label::new(Some("Profile Name:")));
        content_area.append(&entry);

        dialog.show();

        let m_clone = manager.clone();
        let wallpapers_cloned = wallpapers.clone();
        let config_path = config_path.clone();
        // Handle responses
        dialog.connect_response(move |d, resp| {
            if resp == gtk::ResponseType::Ok {
                let name = entry.text().to_string();
                if !name.trim().is_empty() {
                    println!("Creating profile: {}", name);
                    m_clone.borrow_mut().create_profile(name.as_str());

                    for wallpaper in wallpapers_cloned.borrow().iter()
                    {
                        m_clone.borrow_mut().set_wallpaper_in_profile(name.as_str(), wallpaper.monitor_name.as_str(), wallpaper.filename.as_str());
                    }
                    m_clone.borrow_mut().save_config(&config_path);
                }
            }
            d.close();
        });
    });

    let selector_and_new_profile = Box::new(gtk::Orientation::Horizontal, 5);
    selector_and_new_profile.append(&profile_selector);
    selector_and_new_profile.append(&new_button);

    v_box.append(&selector_and_new_profile);
    v_box.append(&warning_alert.container);

    v_box.append(&grid);
    v_box.append(&apply_button);
    window.set_child(Some(&v_box));

    window.present();
}

fn update_warning_alert(dropdown: &DropDown, alert: &AlertBox, manager: &Rc<RefCell<WallpaperManager>>) {
    if let Some(selected_item) = dropdown.selected_item() {
        if let Ok(string_object) = selected_item.downcast::<gtk::StringObject>() {
            let selected_text = string_object.string();
            let mgr = manager.borrow();
            let mut parts = Vec::new();

            if let Some(info) = mgr.check_profile_mismatch(selected_text.as_str()) {
                if !info.extra_aliases.is_empty() {
                    parts.push(format!("Profile has unknown aliases: {}", info.extra_aliases.join(", ")));
                }
                if !info.missing_aliases.is_empty() {
                    parts.push(format!("No wallpaper set for: {}", info.missing_aliases.join(", ")));
                }
            }

            // Check for missing image files
            if let Some(profile) = mgr.profiles.get(selected_text.as_str()) {
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
                return;
            }
        }
    }
    alert.hide();
}

fn update_wallpaper_images_from_profile(dropdown: &DropDown,
                                        wallpapers: &Vec<WallpaperData>,
                                        manager: &Rc<RefCell<WallpaperManager>>) {
    if let Some(selected_item) = dropdown.selected_item() {
        if let Ok(string_object) = selected_item.downcast::<gtk::StringObject>() {
            let selected_text = string_object.string();
            let mgr = manager.borrow();
            let profile = mgr.profiles.get(selected_text.as_str());

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
    }
}

// Example usage and CLI interface
fn main() {
    let config_path = std::env::args().nth(1).unwrap_or_else(|| "config.json".to_string());

    let app = Application::builder()
        .application_id("org.example.HelloWorld")
        .build();

    app.connect_activate(move |app| {
        build_ui(app, &config_path);
    });

    app.run(); //blocks

    /*let mut manager : WallpaperManager = WallpaperManager::new();
    manager.load_config("config.json");

    println!("Rust Wallpaper Manager");
    println!("======================");

    loop {
        println!("\nCommands:");
        println!("1. monitors     - List available monitors");
        println!("2. create       - Create new profile");
        println!("3. set          - Set wallpaper for monitor in profile");
        println!("4. apply        - Apply profile");
        println!("5. profiles     - List profiles");
        println!("6. schedule     - Add schedule");
        println!("7. schedules    - List schedules");
        println!("8. start_sched  - Start scheduler");
        println!("9. stop_sched   - Stop scheduler");
        println!("10. save        - Save configuration");
        println!("11. load        - Load configuration");
        println!("12. quit        - Exit program");

        print!("\nEnter command: ");
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let command = input.trim();

        match command {
            "1" | "monitors" => {
                manager.print_monitors();
            }
            "2" | "create" => {
                print!("Enter profile name: ");
                std::io::stdout().flush().unwrap();
                let mut profile_name = String::new();
                std::io::stdin().read_line(&mut profile_name).unwrap();
                manager.create_profile(profile_name.trim());
            }
            "3" | "set" => {
                print!("Enter profile name: ");
                std::io::stdout().flush().unwrap();
                let mut profile_name = String::new();
                std::io::stdin().read_line(&mut profile_name).unwrap();

                print!("Enter device name: ");
                std::io::stdout().flush().unwrap();
                let mut device_name = String::new();
                std::io::stdin().read_line(&mut device_name).unwrap();

                print!("Enter wallpaper path: ");
                std::io::stdout().flush().unwrap();
                let mut wallpaper_path = String::new();
                std::io::stdin().read_line(&mut wallpaper_path).unwrap();

                manager.set_wallpaper_in_profile(
                    profile_name.trim(),
                    device_name.trim(),
                    wallpaper_path.trim()
                );
            }
            "4" | "apply" => {
                print!("Enter profile name to apply: ");
                std::io::stdout().flush().unwrap();
                let mut profile_name = String::new();
                std::io::stdin().read_line(&mut profile_name).unwrap();
                manager.apply_profile(profile_name.trim());
            }
            "5" | "profiles" => {
                manager.list_profiles();
            }
            "6" | "schedule" => {
                print!("Enter profile name: ");
                std::io::stdout().flush().unwrap();
                let mut profile_name = String::new();
                std::io::stdin().read_line(&mut profile_name).unwrap();

                print!("Enter hour (0-23): ");
                std::io::stdout().flush().unwrap();
                let mut hour_str = String::new();
                std::io::stdin().read_line(&mut hour_str).unwrap();

                print!("Enter minute (0-59): ");
                std::io::stdout().flush().unwrap();
                let mut minute_str = String::new();
                std::io::stdin().read_line(&mut minute_str).unwrap();

                if let (Ok(hour), Ok(minute)) = (hour_str.trim().parse(), minute_str.trim().parse()) {
                    manager.add_schedule(profile_name.trim(), hour, minute);
                } else {
                    println!("Invalid time format!");
                }
            }
            "7" | "schedules" => {
                manager.list_schedule();
            }
            "8" | "start_sched" => {
                manager.start_scheduler();
            }
            "9" | "stop_sched" => {
                manager.stop_scheduler();
            }
            "10" | "save" => {
                print!("Enter config filename: ");
                std::io::stdout().flush().unwrap();
                let mut filename = String::new();
                std::io::stdin().read_line(&mut filename).unwrap();
                manager.save_config(filename.trim());
            }
            "11" | "load" => {
                print!("Enter config filename: ");
                std::io::stdout().flush().unwrap();
                let mut filename = String::new();
                std::io::stdin().read_line(&mut filename).unwrap();
                manager.load_config(filename.trim());
            }
            "12" | "quit" | "exit" => {
                manager.stop_scheduler();
                println!("Goodbye!");
                break;
            }
            _ => {
                println!("Unknown command: {}", command);
            }
        }
    }*/
}