#![cfg(target_os = "linux")]

use super::{MonitorInfo, WallpaperBackend};

use log::{info, warn, error};
use std::collections::HashMap;
use std::process::Command;

pub struct KdeBackend {
    screen_map: HashMap<String, i32>,
}

impl KdeBackend {
    pub fn new() -> Self {
        KdeBackend {
            screen_map: HashMap::new(),
        }
    }

    fn qdbus_cmd() -> &'static str {
        if Command::new("qdbus6").arg("--version").output().is_ok() {
            "qdbus6"
        } else {
            "qdbus"
        }
    }

    fn parse_kscreen_outputs() -> Vec<MonitorInfo> {
        let output = match Command::new("kscreen-doctor").arg("--outputs").output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            Ok(o) => {
                warn!(
                    "kscreen-doctor failed with exit code {:?}, falling back to sysfs",
                    o.status.code()
                );
                return Self::parse_sysfs_outputs();
            }
            Err(e) => {
                warn!("Failed to run kscreen-doctor: {}, falling back to sysfs", e);
                return Self::parse_sysfs_outputs();
            }
        };

        let mut monitors = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_width: u32 = 0;
        let mut current_height: u32 = 0;
        let mut current_primary = false;
        let mut current_x: i32 = 0;
        let mut current_y: i32 = 0;

        for line in output.lines() {
            let trimmed = line.trim();

            // Output lines look like: "Output: 1 DP-1 enabled connected primary"
            if trimmed.starts_with("Output:") {
                // Save previous monitor if any
                if let Some(name) = current_name.take() {
                    monitors.push((name, current_width, current_height, current_primary, current_x, current_y));
                }

                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                // parts: ["Output:", "1", "DP-1", "enabled", "connected", ...]
                if parts.len() >= 3 {
                    current_name = Some(parts[2].to_string());
                    current_primary = parts.iter().any(|&p| p == "primary");
                }
                current_width = 0;
                current_height = 0;
                current_x = 0;
                current_y = 0;
            }

            // Geometry line: "Geometry: 0,0 2560x1440"
            if trimmed.starts_with("Geometry:") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                // parts: ["Geometry:", "0,0", "2560x1440"]
                if parts.len() >= 3 {
                    if let Some((pos_x, pos_y)) = parts[1].trim_end_matches(',').split_once(',') {
                        // Handle trailing comma in position (some formats: "0,0" or "0,0,")
                        let pos_str = parts[1];
                        let pos_parts: Vec<&str> = pos_str.split(',').collect();
                        if pos_parts.len() >= 2 {
                            current_x = pos_parts[0].parse().unwrap_or(0);
                            current_y = pos_parts[1].parse().unwrap_or(0);
                        }
                    }

                    if let Some((w, h)) = parts[2].split_once('x') {
                        current_width = w.parse().unwrap_or(0);
                        current_height = h.parse().unwrap_or(0);
                    }
                }
            }
        }

        // Don't forget the last monitor
        if let Some(name) = current_name.take() {
            monitors.push((name, current_width, current_height, current_primary, current_x, current_y));
        }

        let result: Vec<MonitorInfo> = monitors.into_iter().map(|(name, w, h, primary, _x, _y)| {
            MonitorInfo {
                device_name: name,
                width: w,
                height: h,
                is_primary: primary,
            }
        }).collect();

        // If kscreen-doctor ran but returned no monitors, try sysfs fallback
        if result.is_empty() {
            warn!("kscreen-doctor returned no monitors, falling back to sysfs");
            return Self::parse_sysfs_outputs();
        }

        result
    }

    /// Fallback monitor detection using sysfs when kscreen-doctor fails.
    /// This reads from /sys/class/drm/card*-* to find connected displays.
    fn parse_sysfs_outputs() -> Vec<MonitorInfo> {
        use std::fs;
        use std::path::Path;

        let mut monitors = Vec::new();
        let drm_path = Path::new("/sys/class/drm");

        let entries = match fs::read_dir(drm_path) {
            Ok(e) => e,
            Err(e) => {
                error!("Failed to read /sys/class/drm: {}", e);
                return monitors;
            }
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip entries that don't match card*-* pattern (e.g., "card1-DP-1")
            if !name.starts_with("card") || !name.contains('-') {
                continue;
            }

            // Extract connector name (e.g., "DP-1" from "card1-DP-1")
            let connector_name = match name.split_once('-') {
                Some((_, connector)) => connector.to_string(),
                None => continue,
            };

            let dir_path = entry.path();

            // Check if connected
            let status_path = dir_path.join("status");
            let status = fs::read_to_string(&status_path).unwrap_or_default();
            if status.trim() != "connected" {
                continue;
            }

            // Parse first mode for resolution (e.g., "1920x1080")
            let modes_path = dir_path.join("modes");
            let modes_content = fs::read_to_string(&modes_path).unwrap_or_default();
            let first_mode = modes_content.lines().next().unwrap_or("");

            let (width, height) = if let Some((w, h)) = first_mode.split_once('x') {
                (w.parse().unwrap_or(0), h.parse().unwrap_or(0))
            } else {
                (0, 0)
            };

            monitors.push(MonitorInfo {
                device_name: connector_name,
                width,
                height,
                is_primary: false, // Cannot determine primary from sysfs alone
            });
        }

        // Sort by connector name for consistent ordering
        monitors.sort_by(|a, b| a.device_name.cmp(&b.device_name));

        // Mark first monitor as primary if we have any
        if !monitors.is_empty() {
            monitors[0].is_primary = true;
        }

        info!("Detected {} monitors via sysfs fallback", monitors.len());
        monitors
    }

    fn build_screen_map(monitors: &[MonitorInfo]) -> HashMap<String, i32> {
        let mut map = HashMap::new();

        // Assign monitors to screen indices sequentially.
        for (i, monitor) in monitors.iter().enumerate() {
            map.insert(monitor.device_name.clone(), i as i32);
        }

        map
    }
}

impl WallpaperBackend for KdeBackend {
    fn refresh_monitors(&mut self) -> Vec<MonitorInfo> {
        let monitors = Self::parse_kscreen_outputs();
        self.screen_map = Self::build_screen_map(&monitors);
        monitors
    }

    fn get_current_wallpaper(&self, monitor_id: &str) -> String {
        let screen_idx = match self.screen_map.get(monitor_id) {
            Some(&idx) => idx,
            None => 0,
        };

        let qdbus = Self::qdbus_cmd();

        let script = format!(
            "var d = desktops(); for (var i = 0; i < d.length; i++) {{ if (d[i].screen == {}) {{ d[i].currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General']; print(d[i].readConfig('Image', '').replace('file://', '')); break; }} }}",
            screen_idx
        );

        let output = Command::new(qdbus)
            .args([
                "org.kde.plasmashell",
                "/PlasmaShell",
                "org.kde.PlasmaShell.evaluateScript",
                &script,
            ])
            .output();

        match output {
            Ok(o) => {
                let result = String::from_utf8_lossy(&o.stdout).trim().to_string();
                result
            }
            Err(e) => {
                error!("Failed to get wallpaper via qdbus: {}", e);
                String::new()
            }
        }
    }

    fn set_wallpaper(&self, monitor_id: &str, wallpaper_path: &str) -> bool {
        let screen_idx = match self.screen_map.get(monitor_id) {
            Some(&idx) => idx,
            None => 0,
        };

        let qdbus = Self::qdbus_cmd();

        // Ensure path has file:// prefix
        let file_url = if wallpaper_path.starts_with("file://") {
            wallpaper_path.to_string()
        } else {
            format!("file://{}", wallpaper_path)
        };

        let script = format!(
            "var d = desktops(); for (var i = 0; i < d.length; i++) {{ if (d[i].screen == {}) {{ d[i].currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General']; d[i].writeConfig('Image', '{}'); d[i].reloadConfig(); break; }} }}",
            screen_idx, file_url
        );

        let output = Command::new(qdbus)
            .args([
                "org.kde.plasmashell",
                "/PlasmaShell",
                "org.kde.PlasmaShell.evaluateScript",
                &script,
            ])
            .output();

        match output {
            Ok(o) => {
                if o.status.success() {
                    info!("Set wallpaper for {} (screen {})", monitor_id, screen_idx);
                    true
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    error!("Failed to set wallpaper for {}: {}", monitor_id, stderr);
                    false
                }
            }
            Err(e) => {
                error!("Failed to run qdbus: {}", e);
                false
            }
        }
    }
}
