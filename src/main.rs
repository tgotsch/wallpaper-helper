use std::cell::RefCell;
use std::io::Write;
use std::ops::Index;
use std::rc::Rc;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{glib, ApplicationWindow, Application, Button, Box, Image};
use gtk4::glib::property::PropertyGet;
use crate::wallpaper_manager::WallpaperManager;

mod wallpaper_manager;

fn build_ui(app: &gtk::Application) {
    let mut manager = Rc::new(RefCell::new(WallpaperManager::new()));
    manager.borrow_mut().load_config("config.txt");

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(350)
        .default_height(200)
        .title("Wallpaper")
        .build();

    let v_box = Box::new(gtk::Orientation::Vertical, 10);
    let grid = gtk::Grid::new();
    grid.set_column_spacing(5);

    let mut wallpapers: Vec<Image> = Vec::new();

    for (i, monitor) in manager.borrow().monitors.iter().enumerate()
    {
        let current_wallpaper = Image::builder()
            .file(manager.borrow().get_current_wallpaper_by_monitor_id(monitor.device_name.as_str()))
            .hexpand(true)
            .vexpand(true)
            .build();

        wallpapers.push(current_wallpaper.clone());

        let button = Button::with_label(&*monitor.device_name);
        button.connect_clicked(move |_| {
            eprintln!("Clicked!");
        });

        let label = gtk::Label::new(Some(&monitor.device_name));

        grid.attach(&label, i as i32, 0, 1, 1);
        grid.attach(&current_wallpaper, i as i32, 1, 1, 1);
    }

    let foo : Vec<String> = manager.borrow().profiles.keys().cloned().collect();
    let foo_strings: Vec<&str> = foo.iter().map(|s| s.as_str()).collect();
    let profile_selector = gtk::DropDown::from_strings(foo_strings.as_slice());

    profile_selector.connect_selected_notify({
        let dropdown = profile_selector.clone();
        let wallpapers_cloned = wallpapers.clone();
        let manager_clone = manager.clone();
        move |_| {
            if let Some(selected_item) = dropdown.selected_item()
            {
                if let Ok(string_object) = selected_item.downcast::<gtk::StringObject>()
                {
                    let selected_text = string_object.string();
                    if let Some(selected_profile) = manager_clone.borrow().profiles.get(selected_text.as_str())
                    {
                        for (i, pair) in selected_profile.monitor_wallpapers.iter().enumerate()
                        {
                            wallpapers_cloned[i].set_from_file(Some(pair.1));
                        }
                    }
                }
            }
        }
    });

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
    let mut manager_clone = manager.clone();
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
        // Handle responses
        dialog.connect_response(move |d, resp| {
            if resp == gtk::ResponseType::Ok {
                let name = entry.text().to_string();
                if !name.trim().is_empty() {
                    println!("Creating profile: {}", name);
                    m_clone.borrow_mut().create_profile(name.as_str());
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

// Example usage and CLI interface
fn main() {
    let app = Application::builder()
        .application_id("org.example.HelloWorld")
        .build();

    app.connect_activate(build_ui);

    app.run(); //blocks

    /*println!("Rust Wallpaper Manager");
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