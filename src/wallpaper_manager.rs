use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use chrono::{Local, Timelike};
use serde::{Serialize, Deserialize};

use crate::backend::{MonitorInfo, WallpaperBackend, create_backend};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperProfile {
    #[serde(skip)]
    pub name: String,
    pub monitor_wallpapers: HashMap<String, String>, // deviceName -> wallpaperPath
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub profile_name: String,
    pub hour: u32,
    pub minute: u32,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    profiles: HashMap<String, WallpaperProfile>,
    schedule: Vec<ScheduleEntry>,
}

pub struct WallpaperManager {
    pub monitors: Vec<MonitorInfo>,
    pub profiles: HashMap<String, WallpaperProfile>,
    pub schedule: Vec<ScheduleEntry>,
    pub scheduler_running: Arc<AtomicBool>,
    backend: Box<dyn WallpaperBackend>,
}

impl WallpaperManager {
    pub fn new() -> Self {
        let mut backend = create_backend();
        let monitors = backend.refresh_monitors();

        println!("\n=== Monitor Information ===");
        println!("Found {} monitors:", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            println!("  {}. {}{} - {}x{}",
                     i + 1,
                     monitor.device_name,
                     if monitor.is_primary { " (Primary)" } else { "" },
                     monitor.width,
                     monitor.height,
            );
        }
        println!("===========================\n");

        Self {
            monitors,
            profiles: HashMap::new(),
            schedule: Vec::new(),
            scheduler_running: Arc::new(AtomicBool::new(false)),
            backend,
        }
    }

    fn refresh_monitors(&mut self) {
        self.monitors = self.backend.refresh_monitors();

        println!("\n=== Monitor Information ===");
        println!("Found {} monitors:", self.monitors.len());
        for (i, monitor) in self.monitors.iter().enumerate() {
            println!("  {}. {}{} - {}x{}",
                     i + 1,
                     monitor.device_name,
                     if monitor.is_primary { " (Primary)" } else { "" },
                     monitor.width,
                     monitor.height,
            );
        }
        println!("===========================\n");
    }

    pub fn get_current_wallpaper_by_monitor_id(&self, monitor_id: &str) -> String {
        self.backend.get_current_wallpaper(monitor_id)
    }

    pub fn print_monitors(&mut self) {
        self.refresh_monitors();

        println!("Available monitors for wallpaper setting:");
        println!("==========================================");

        for (i, monitor) in self.monitors.iter().enumerate() {
            println!("{}. {}{} - {}x{}",
                     i + 1,
                     monitor.device_name,
                     if monitor.is_primary { " (Primary)" } else { "" },
                     monitor.width,
                     monitor.height,
            );
            println!("   Use device name: {}", monitor.device_name);

            let current_wallpaper = self.backend.get_current_wallpaper(&monitor.device_name);
            if !current_wallpaper.is_empty() {
                println!("   Current wallpaper: {}", current_wallpaper);
            }
            println!();
        }

        println!("==========================================");
        println!("Tip: Copy the 'Use device name' exactly when setting wallpapers.\n");
    }

    pub fn create_profile(&mut self, profile_name: &str) -> bool {
        if self.profiles.contains_key(profile_name) {
            println!("Profile '{}' already exists!", profile_name);
            return false;
        }

        self.profiles.insert(profile_name.to_string(), WallpaperProfile {
            name: profile_name.to_string(),
            monitor_wallpapers: HashMap::new(),
            tags: Vec::new(),
        });

        println!("Profile '{}' created.", profile_name);
        true
    }

    pub fn set_wallpaper_in_profile(&mut self, profile_name: &str, device_name: &str, wallpaper_path: &str) -> bool {
        if !self.profiles.contains_key(profile_name) {
            println!("Profile '{}' not found!", profile_name);
            return false;
        }

        // Verify file exists
        if !Path::new(wallpaper_path).exists() {
            println!("Wallpaper file not found: {}", wallpaper_path);
            return false;
        }

        // Check if it's a supported image format
        if let Some(extension) = Path::new(wallpaper_path).extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tiff") {
                println!("Unsupported image format: {}", ext);
                println!("Supported formats: jpg, jpeg, png, bmp, gif, tiff");
                return false;
            }
        }

        // Verify device name exists
        let device_found = self.monitors.iter().any(|monitor| monitor.device_name == device_name);
        if !device_found {
            println!("Monitor device '{}' not found!", device_name);
            println!("Available monitors:");
            for monitor in &self.monitors {
                println!("  {}", monitor.device_name);
            }
            return false;
        }

        if let Some(profile) = self.profiles.get_mut(profile_name) {
            profile.monitor_wallpapers.insert(device_name.to_string(), wallpaper_path.to_string());
            println!("Added wallpaper to profile '{}' for monitor {}", profile_name, device_name);
            true
        } else {
            false
        }
    }

    pub fn apply_profile(&self, profile_name: &str) -> bool {
        if let Some(profile) = self.profiles.get(profile_name) {
            let mut success = true;
            println!("Applying profile '{}'...", profile_name);

            for (device_name, wallpaper_path) in &profile.monitor_wallpapers {
                if !self.backend.set_wallpaper(device_name, wallpaper_path) {
                    println!("Failed to set wallpaper for {}", device_name);
                    success = false;
                } else {
                    println!("Set wallpaper for {}", device_name);
                }
            }

            success
        } else {
            println!("Profile '{}' not found!", profile_name);
            false
        }
    }

    pub fn list_profiles(&self) -> Vec<String> {
        if self.profiles.is_empty() {
            println!("No profiles created.");
            return Vec::new();
        }

        println!("Available profiles:");
        let mut profile_names = Vec::new();
        for (name, profile) in &self.profiles {
            println!("- {} ({} monitors)", name, profile.monitor_wallpapers.len());
            profile_names.push(name.clone());
        }

        profile_names
    }

    pub fn add_schedule(&mut self, profile_name: &str, hour: u32, minute: u32) -> bool {
        if !self.profiles.contains_key(profile_name) {
            println!("Profile '{}' not found!", profile_name);
            return false;
        }

        if hour > 23 || minute > 59 {
            println!("Invalid time format. Use 24-hour format (0-23 for hours, 0-59 for minutes).");
            return false;
        }

        self.schedule.push(ScheduleEntry {
            profile_name: profile_name.to_string(),
            hour,
            minute,
            enabled: true,
        });

        println!("Scheduled profile '{}' at {:02}:{:02}", profile_name, hour, minute);
        true
    }

    pub fn list_schedule(&self) {
        if self.schedule.is_empty() {
            println!("No scheduled profiles.");
            return;
        }

        println!("Scheduled profiles:");
        for (i, entry) in self.schedule.iter().enumerate() {
            println!("{}. {} at {:02}:{:02}{}",
                     i + 1,
                     entry.profile_name,
                     entry.hour,
                     entry.minute,
                     if entry.enabled { " (enabled)" } else { " (disabled)" }
            );
        }
    }

    pub fn start_scheduler(&mut self) {
        if self.scheduler_running.load(Ordering::Relaxed) {
            println!("Scheduler is already running.");
            return;
        }

        self.scheduler_running.store(true, Ordering::Relaxed);
        let scheduler_running = self.scheduler_running.clone();
        let schedule = self.schedule.clone();
        let _profiles = self.profiles.clone();

        thread::spawn(move || {
            while scheduler_running.load(Ordering::Relaxed) {
                let now = Local::now();
                let current_hour = now.hour();
                let current_minute = now.minute();

                for entry in &schedule {
                    if entry.enabled && entry.hour == current_hour && entry.minute == current_minute {
                        println!("Time to apply profile: {}", entry.profile_name);
                        thread::sleep(Duration::from_secs(60));
                    }
                }

                thread::sleep(Duration::from_secs(30));
            }
        });

        println!("Scheduler started.");
    }

    pub fn stop_scheduler(&mut self) {
        if !self.scheduler_running.load(Ordering::Relaxed) {
            return;
        }

        self.scheduler_running.store(false, Ordering::Relaxed);
        println!("Scheduler stopped.");
    }

    pub fn add_tag(&mut self, tag_name: &str, profile_name: &str) {
        if let Some(found) = self.profiles.get_mut(profile_name) {
            found.tags.push(tag_name.to_string());
        }
    }

    pub fn get_tags(&self, profile_name: &str) -> Vec<String> {
        self.profiles.values().filter(|profile| profile.name == profile_name).map(|profile| profile.tags.clone()).flatten().collect()
    }

    pub fn save_config(&self, filename: &str) -> bool {
        let config = Config {
            profiles: self.profiles.clone(),
            schedule: self.schedule.clone(),
        };

        match serde_json::to_string_pretty(&config) {
            Ok(json) => {
                match fs::write(filename, json) {
                    Ok(_) => {
                        println!("Configuration saved to {}", filename);
                        true
                    }
                    Err(e) => {
                        println!("Failed to save config to {}: {}", filename, e);
                        false
                    }
                }
            }
            Err(e) => {
                println!("Failed to serialize config: {}", e);
                false
            }
        }
    }

    pub fn load_config(&mut self, filename: &str) -> bool {
        let contents = match fs::read_to_string(filename) {
            Ok(contents) => contents,
            Err(_) => {
                println!("Config file not found: {}", filename);
                return false;
            }
        };

        match serde_json::from_str::<Config>(&contents) {
            Ok(config) => {
                self.profiles = config.profiles;
                self.schedule = config.schedule;

                // Populate the skipped `name` field from the HashMap keys
                for (name, profile) in &mut self.profiles {
                    profile.name = name.clone();
                }

                println!("Configuration loaded from {}", filename);
                true
            }
            Err(e) => {
                println!("Failed to parse config from {}: {}", filename, e);
                false
            }
        }
    }
}
