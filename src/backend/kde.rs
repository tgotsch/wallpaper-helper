#![cfg(target_os = "linux")]

use super::{MonitorInfo, WallpaperBackend};

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
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(e) => {
                println!("Failed to run kscreen-doctor: {}", e);
                return Vec::new();
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

        monitors.into_iter().map(|(name, w, h, primary, _x, _y)| {
            MonitorInfo {
                device_name: name,
                width: w,
                height: h,
                is_primary: primary,
            }
        }).collect()
    }

    fn build_screen_map(monitors: &[MonitorInfo]) -> HashMap<String, i32> {
        let qdbus = Self::qdbus_cmd();

        // Query plasma for desktop count and geometry to map connector names to screen indices
        let qdbus_output = Command::new(qdbus)
            .args([
                "org.kde.plasmashell",
                "/PlasmaShell",
                "org.kde.PlasmaShell.evaluateScript",
                "var result = []; for (var i = 0; i < desktops().length; i++) { var d = desktops()[i]; result.push(d.screen + '|' + d.screenGeometry.x + ',' + d.screenGeometry.y + ',' + d.screenGeometry.width + ',' + d.screenGeometry.height); } result.join('\\n');",
            ])
            .output();

        let mut map = HashMap::new();

        let qdbus_str = match qdbus_output {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => {
                // qdbus unavailable, fall back to sequential assignment
                for (i, monitor) in monitors.iter().enumerate() {
                    map.insert(monitor.device_name.clone(), i as i32);
                }
                return map;
            }
        };

        // Parse kscreen-doctor output for connector name -> (x, y) position
        let kscreen_output = Command::new("kscreen-doctor").arg("--outputs").output();
        let mut entries: Vec<(String, i32, i32)> = Vec::new();

        if let Ok(o) = kscreen_output {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            let mut cur_name: Option<String> = None;
            let mut cur_x: i32 = 0;
            let mut cur_y: i32 = 0;

            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Output:") {
                    if let Some(name) = cur_name.take() {
                        entries.push((name, cur_x, cur_y));
                    }
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 3 {
                        cur_name = Some(parts[2].to_string());
                    }
                    cur_x = 0;
                    cur_y = 0;
                }
                if trimmed.starts_with("Geometry:") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let pos_parts: Vec<&str> = parts[1].split(',').collect();
                        if pos_parts.len() >= 2 {
                            cur_x = pos_parts[0].parse().unwrap_or(0);
                            cur_y = pos_parts[1].parse().unwrap_or(0);
                        }
                    }
                }
            }
            if let Some(name) = cur_name.take() {
                entries.push((name, cur_x, cur_y));
            }
        }

        // Match qdbus screen indices to kscreen connector names by geometry position
        // Each qdbus line: "screen_index|x,y,w,h"
        for line in qdbus_str.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((screen_str, geom_str)) = line.split_once('|') {
                let screen_idx: i32 = match screen_str.parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let geom_parts: Vec<&str> = geom_str.split(',').collect();
                if geom_parts.len() < 2 {
                    continue;
                }
                let gx: i32 = geom_parts[0].parse().unwrap_or(-1);
                let gy: i32 = geom_parts[1].parse().unwrap_or(-1);

                for (name, kx, ky) in &entries {
                    if *kx == gx && *ky == gy {
                        map.insert(name.clone(), screen_idx);
                        break;
                    }
                }
            }
        }

        // If mapping is still empty, fall back to sequential assignment
        if map.is_empty() {
            for (i, monitor) in monitors.iter().enumerate() {
                map.insert(monitor.device_name.clone(), i as i32);
            }
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
            "var d = desktops(); for (var i = 0; i < d.length; i++) {{ if (d[i].screen == {}) {{ print(d[i].readConfig('Image', '').replace('file://', '')); break; }} }}",
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
                println!("Failed to get wallpaper via qdbus: {}", e);
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
            "var d = desktops(); for (var i = 0; i < d.length; i++) {{ if (d[i].screen == {}) {{ d[i].writeConfig('Image', '{}'); d[i].reloadConfig(); break; }} }}",
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
                    println!("Set wallpaper for {} (screen {})", monitor_id, screen_idx);
                    true
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    println!("Failed to set wallpaper for {}: {}", monitor_id, stderr);
                    false
                }
            }
            Err(e) => {
                println!("Failed to run qdbus: {}", e);
                false
            }
        }
    }
}
