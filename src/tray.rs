pub enum TrayAction {
    ShowWindow,
    Quit,
    NextWallpaper,
    PrevWallpaper,
    ToggleSlideshow,
}

#[cfg(windows)]
mod platform {
    use super::TrayAction;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub struct AppTray {
        _tray: TrayIcon,
        show_id: tray_icon::menu::MenuId,
        quit_id: tray_icon::menu::MenuId,
        next_id: tray_icon::menu::MenuId,
        prev_id: tray_icon::menu::MenuId,
        toggle_id: tray_icon::menu::MenuId,
        status_item: MenuItem,
        next_item: MenuItem,
        prev_item: MenuItem,
        toggle_item: MenuItem,
    }

    impl AppTray {
        pub fn new() -> Self {
            // Create a 16x16 teal icon (RGBA)
            let mut rgba = Vec::with_capacity(16 * 16 * 4);
            for _ in 0..(16 * 16) {
                rgba.extend_from_slice(&[0, 160, 160, 255]);
            }
            let icon = Icon::from_rgba(rgba, 16, 16).expect("Failed to create tray icon");

            let status_item = MenuItem::new("Slideshow: inactive", false, None);
            let prev_item = MenuItem::new("Previous Wallpaper", false, None);
            let next_item = MenuItem::new("Next Wallpaper", false, None);
            let toggle_item = MenuItem::new("Resume Slideshow", false, None);
            let show_item = MenuItem::new("Show Window", true, None);
            let quit_item = MenuItem::new("Quit", true, None);

            let prev_id = prev_item.id().clone();
            let next_id = next_item.id().clone();
            let toggle_id = toggle_item.id().clone();
            let show_id = show_item.id().clone();
            let quit_id = quit_item.id().clone();

            let menu = Menu::new();
            menu.append(&status_item).unwrap();
            menu.append(&prev_item).unwrap();
            menu.append(&next_item).unwrap();
            menu.append(&toggle_item).unwrap();
            menu.append(&PredefinedMenuItem::separator()).unwrap();
            menu.append(&show_item).unwrap();
            menu.append(&quit_item).unwrap();

            let tray = TrayIconBuilder::new()
                .with_tooltip("Wallpaper Helper")
                .with_icon(icon)
                .with_menu(Box::new(menu))
                .build()
                .expect("Failed to build tray icon");

            Self {
                _tray: tray,
                show_id,
                quit_id,
                next_id,
                prev_id,
                toggle_id,
                status_item,
                next_item,
                prev_item,
                toggle_item,
            }
        }

        pub fn update_slideshow_state(&self, active: bool, collection_name: &str) {
            let has_collection = !collection_name.is_empty();
            if active {
                self.status_item.set_text(&format!("Slideshow: {} (active)", collection_name));
                self.toggle_item.set_text("Stop Slideshow");
                self.toggle_item.set_enabled(true);
            } else if has_collection {
                self.status_item.set_text(&format!("Slideshow: {} (paused)", collection_name));
                self.toggle_item.set_text("Resume Slideshow");
                self.toggle_item.set_enabled(true);
            } else {
                self.status_item.set_text("Slideshow: inactive");
                self.toggle_item.set_text("Resume Slideshow");
                self.toggle_item.set_enabled(false);
            }
            self.prev_item.set_enabled(has_collection);
            self.next_item.set_enabled(has_collection);
        }

        pub fn poll_actions(&self) -> Vec<TrayAction> {
            let mut actions = Vec::new();

            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == self.show_id {
                    actions.push(TrayAction::ShowWindow);
                } else if event.id == self.quit_id {
                    actions.push(TrayAction::Quit);
                } else if event.id == self.next_id {
                    actions.push(TrayAction::NextWallpaper);
                } else if event.id == self.prev_id {
                    actions.push(TrayAction::PrevWallpaper);
                } else if event.id == self.toggle_id {
                    actions.push(TrayAction::ToggleSlideshow);
                }
            }

            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                    actions.push(TrayAction::ShowWindow);
                }
            }

            actions
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::TrayAction;
    use ksni::blocking::TrayMethods;
    use std::sync::{mpsc, Arc, Mutex};

    struct KsniTray {
        sender: mpsc::Sender<TrayAction>,
        slideshow_state: Arc<Mutex<(bool, String)>>,
    }

    impl ksni::Tray for KsniTray {
        fn id(&self) -> String {
            "wallpaper-helper".into()
        }

        fn title(&self) -> String {
            "Wallpaper Helper".into()
        }

        fn icon_name(&self) -> String {
            "preferences-desktop-wallpaper".into()
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            let (active, ref col_name) = *self.slideshow_state.lock().unwrap();
            let has_collection = !col_name.is_empty();

            let status_label = if active {
                format!("Slideshow: {} (active)", col_name)
            } else if has_collection {
                format!("Slideshow: {} (paused)", col_name)
            } else {
                "Slideshow: inactive".into()
            };

            let toggle_label = if active { "Stop Slideshow" } else { "Resume Slideshow" };

            vec![
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: status_label,
                    enabled: false,
                    ..Default::default()
                }),
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: "Previous Wallpaper".into(),
                    enabled: has_collection,
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send(TrayAction::PrevWallpaper);
                    }),
                    ..Default::default()
                }),
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: "Next Wallpaper".into(),
                    enabled: has_collection,
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send(TrayAction::NextWallpaper);
                    }),
                    ..Default::default()
                }),
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: toggle_label.into(),
                    enabled: active || has_collection,
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send(TrayAction::ToggleSlideshow);
                    }),
                    ..Default::default()
                }),
                ksni::MenuItem::Separator,
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: "Show Window".into(),
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send(TrayAction::ShowWindow);
                    }),
                    ..Default::default()
                }),
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: "Quit".into(),
                    activate: Box::new(|tray: &mut Self| {
                        let _ = tray.sender.send(TrayAction::Quit);
                    }),
                    ..Default::default()
                }),
            ]
        }
    }

    pub struct AppTray {
        receiver: mpsc::Receiver<TrayAction>,
        slideshow_state: Arc<Mutex<(bool, String)>>,
    }

    impl AppTray {
        pub fn new() -> Self {
            let (sender, receiver) = mpsc::channel();
            let slideshow_state = Arc::new(Mutex::new((false, String::new())));
            let tray = KsniTray {
                sender,
                slideshow_state: slideshow_state.clone(),
            };
            tray.spawn().expect("Failed to spawn tray");

            Self { receiver, slideshow_state }
        }

        pub fn update_slideshow_state(&self, active: bool, collection_name: &str) {
            let mut state = self.slideshow_state.lock().unwrap();
            *state = (active, collection_name.to_string());
        }

        pub fn poll_actions(&self) -> Vec<TrayAction> {
            let mut actions = Vec::new();
            while let Ok(action) = self.receiver.try_recv() {
                actions.push(action);
            }
            actions
        }
    }
}

pub use platform::AppTray;
