pub enum TrayAction {
    ShowWindow,
    Quit,
}

#[cfg(windows)]
mod platform {
    use super::TrayAction;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub struct AppTray {
        _tray: TrayIcon,
        show_id: tray_icon::menu::MenuId,
        quit_id: tray_icon::menu::MenuId,
    }

    impl AppTray {
        pub fn new() -> Self {
            // Create a 16x16 teal icon (RGBA)
            let mut rgba = Vec::with_capacity(16 * 16 * 4);
            for _ in 0..(16 * 16) {
                rgba.extend_from_slice(&[0, 160, 160, 255]);
            }
            let icon = Icon::from_rgba(rgba, 16, 16).expect("Failed to create tray icon");

            let show_item = MenuItem::new("Show Window", true, None);
            let quit_item = MenuItem::new("Quit", true, None);
            let show_id = show_item.id().clone();
            let quit_id = quit_item.id().clone();

            let menu = Menu::new();
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
            }
        }

        pub fn poll_actions(&self) -> Vec<TrayAction> {
            let mut actions = Vec::new();

            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == self.show_id {
                    actions.push(TrayAction::ShowWindow);
                } else if event.id == self.quit_id {
                    actions.push(TrayAction::Quit);
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
    use std::sync::mpsc;

    struct KsniTray {
        sender: mpsc::Sender<TrayAction>,
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
            vec![
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
    }

    impl AppTray {
        pub fn new() -> Self {
            let (sender, receiver) = mpsc::channel();
            let tray = KsniTray { sender };
            tray.spawn().expect("Failed to spawn tray");

            Self { receiver }
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
