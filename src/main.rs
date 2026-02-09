use std::cell::RefCell;
use std::rc::Rc;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{ApplicationWindow, Application, Button, Box, Image, FileChooserDialog};
use gtk4::DropDown;
use crate::wallpaper_manager::WallpaperManager;

mod wallpaper_manager;
//mod wallpaper_source;

#[derive(Clone)]
struct WallpaperData {
    monitor_name: String,
    filename:  String,
    image: Image,
    button: Button,
}

impl WallpaperData {
    fn new(monitor_name: String, initial_filename: String) -> Self {
        let image = Image::new();
        let button = Button::new();

        // Set initial image if file exists
        if !initial_filename.is_empty() {
            image.set_from_file(Some(&initial_filename));
        }

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
                            image_clone.set_from_file(Some(&path_str));

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

fn build_ui(app: &gtk::Application) {
    let manager = Rc::new(RefCell::new(WallpaperManager::new()));
    let wallpapers: Rc<RefCell<Vec<WallpaperData>>> = Rc::new(RefCell::new(Vec::new()));
    manager.borrow_mut().load_config("config.json");

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1000)
        .default_height(1000)
        .title("Wallpaper Helper")
        .build();

    let v_box = Box::new(gtk::Orientation::Vertical, 10);
    let grid = gtk::Grid::new();
    grid.set_column_spacing(5);

    for (i, monitor) in manager.borrow().monitors.iter().enumerate()
    {
        let wallpaper_str = manager.borrow().get_current_wallpaper_by_monitor_id(monitor.device_name.as_str());

        let wallpaper_data = WallpaperData::new(monitor.device_name.clone(), wallpaper_str.to_string());
        wallpaper_data.setup_click_handler(wallpapers.clone());

        wallpapers.borrow_mut().push(wallpaper_data.clone());

        let button = Button::with_label(&*monitor.device_name);
        button.connect_clicked(move |_| {
            eprintln!("Clicked!");
        });

        let label = gtk::Label::new(Some(&monitor.device_name));

        grid.attach(&label, i as i32, 0, 1, 1);
        grid.attach(&wallpaper_data.button, i as i32, 1, 1, 1);
    }

    let foo : Vec<String> = manager.borrow().profiles.keys().cloned().collect();
    let foo_strings: Vec<&str> = foo.iter().map(|s| s.as_str()).collect();
    let profile_selector = gtk::DropDown::from_strings(foo_strings.as_slice());

    profile_selector.connect_selected_notify({
        let dropdown = profile_selector.clone();
        let wallpapers_cloned = wallpapers.clone();
        let manager_clone = manager.clone();
        move |_| {
            update_wallpaper_images_from_profile(&dropdown, &wallpapers_cloned.borrow(), &manager_clone);
        }
    });

    update_wallpaper_images_from_profile(&profile_selector, &wallpapers.borrow(), &manager);

    let apply_button = Button::with_label("Apply Selected profile");
    apply_button.connect_clicked({
        let dropdown = profile_selector.clone();
        let manager_clone = manager.clone();
        move |_|
            {
                if let Some(selected_item) = dropdown.selected_item()
                {
                    if let Ok(string_object) = selected_item.downcast::<gtk::StringObject>()
                    {
                        let selected_text = string_object.string();
                        manager_clone.borrow_mut().apply_profile(&selected_text);
                    }
                }
            }
    });

    let new_button = Button::with_label("New profile");
    let parent_clone = window.clone();
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
                    m_clone.borrow_mut().save_config("config.json");
                }
            }
            d.close();
        });
    });

    let selector_and_new_profile = Box::new(gtk::Orientation::Horizontal, 5);
    selector_and_new_profile.append(&profile_selector);
    selector_and_new_profile.append(&new_button);

    v_box.append(&selector_and_new_profile);

    v_box.append(&grid);
    v_box.append(&apply_button);
    window.set_child(Some(&v_box));

    window.present();
}

fn update_wallpaper_images_from_profile(dropdown: &DropDown,
                                        wallpapers: &Vec<WallpaperData>,
                                        manager: &Rc<RefCell<WallpaperManager>>) {
    if let Some(selected_item) = dropdown.selected_item() {
        if let Ok(string_object) = selected_item.downcast::<gtk::StringObject>() {
            let selected_text = string_object.string();
            if let Some(selected_profile) = manager.borrow().profiles.get(selected_text.as_str()) {
                for (i, pair) in selected_profile.monitor_wallpapers.iter().enumerate() {
                    let found_res = wallpapers.iter().find(|w| w.monitor_name == *pair.0);

                    if let Some(found) = found_res {
                        found.image.set_from_file(Some(pair.1));
                    }
                }
            }
        }
    }
}

// Example usage and CLI interface
fn main() {
    let app = Application::builder()
        .application_id("org.example.HelloWorld")
        .build();

    app.connect_activate(build_ui);

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