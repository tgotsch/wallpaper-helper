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
    pub monitor_wallpapers: HashMap<String, String>, // alias -> relative wallpaper path
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub profile_name: String,
    pub hour: u32,
    pub minute: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub wallpaper_base_path: String,
    pub monitor_map: HashMap<String, String>, // alias -> platform-specific device name
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfigs {
    #[serde(default)]
    pub windows: PlatformConfig,
    #[serde(default)]
    pub linux: PlatformConfig,
}

impl Default for PlatformConfigs {
    fn default() -> Self {
        Self {
            windows: PlatformConfig::default(),
            linux: PlatformConfig::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    platform_config: PlatformConfigs,
    profiles: HashMap<String, WallpaperProfile>,
    schedule: Vec<ScheduleEntry>,
}

#[derive(Debug, Deserialize)]
struct LegacyConfig {
    profiles: HashMap<String, WallpaperProfile>,
    schedule: Vec<ScheduleEntry>,
}

pub struct WallpaperManager {
    pub monitors: Vec<MonitorInfo>,
    pub aliases: Vec<String>,
    pub platform_configs: PlatformConfigs,
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
            aliases: Vec::new(),
            platform_configs: PlatformConfigs::default(),
            profiles: HashMap::new(),
            schedule: Vec::new(),
            scheduler_running: Arc::new(AtomicBool::new(false)),
            backend,
        }
    }

    fn current_platform_config(&self) -> &PlatformConfig {
        #[cfg(windows)]
        { &self.platform_configs.windows }
        #[cfg(target_os = "linux")]
        { &self.platform_configs.linux }
    }

    fn resolve_alias_to_device(&self, alias: &str) -> Option<&String> {
        self.current_platform_config().monitor_map.get(alias)
    }

    pub fn resolve_wallpaper_path(&self, relative_path: &str) -> String {
        if relative_path.is_empty() {
            return String::new();
        }
        let base = &self.current_platform_config().wallpaper_base_path;
        if base.is_empty() {
            return relative_path.to_string();
        }
        Path::new(base).join(relative_path).to_string_lossy().to_string()
    }

    pub fn make_relative_path(&self, absolute_path: &str) -> String {
        let base = &self.current_platform_config().wallpaper_base_path;
        if base.is_empty() {
            return absolute_path.to_string();
        }
        match Path::new(absolute_path).strip_prefix(Path::new(base)) {
            Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
            Err(_) => absolute_path.to_string(),
        }
    }

    pub fn get_alias_monitor_info(&self) -> Vec<(String, Option<MonitorInfo>)> {
        self.aliases.iter().map(|alias| {
            let monitor_info = self.resolve_alias_to_device(alias)
                .and_then(|dev| self.monitors.iter().find(|m| m.device_name == *dev))
                .cloned();
            (alias.clone(), monitor_info)
        }).collect()
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

    pub fn get_current_wallpaper_by_alias(&self, alias: &str) -> String {
        match self.resolve_alias_to_device(alias) {
            Some(device) => self.backend.get_current_wallpaper(device),
            None => {
                println!("No device mapping found for alias '{}'", alias);
                String::new()
            }
        }
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

    pub fn set_wallpaper_in_profile(&mut self, profile_name: &str, alias: &str, wallpaper_path: &str) -> bool {
        if !self.profiles.contains_key(profile_name) {
            println!("Profile '{}' not found!", profile_name);
            return false;
        }

        // Verify file exists (absolute path from file chooser)
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

        // Validate alias exists
        if !self.aliases.contains(&alias.to_string()) {
            println!("Monitor alias '{}' not found!", alias);
            println!("Available aliases:");
            for a in &self.aliases {
                println!("  {}", a);
            }
            return false;
        }

        // Convert absolute path to relative for storage
        let relative_path = self.make_relative_path(wallpaper_path);

        if let Some(profile) = self.profiles.get_mut(profile_name) {
            profile.monitor_wallpapers.insert(alias.to_string(), relative_path);
            println!("Added wallpaper to profile '{}' for monitor {}", profile_name, alias);
            true
        } else {
            false
        }
    }

    pub fn apply_profile(&self, profile_name: &str) -> bool {
        if let Some(profile) = self.profiles.get(profile_name) {
            let mut success = true;
            println!("Applying profile '{}'...", profile_name);

            for (alias, relative_path) in &profile.monitor_wallpapers {
                let device_name = match self.resolve_alias_to_device(alias) {
                    Some(dev) => dev.clone(),
                    None => {
                        println!("No device mapping found for alias '{}' on this platform", alias);
                        success = false;
                        continue;
                    }
                };

                let absolute_path = self.resolve_wallpaper_path(relative_path);

                if !self.backend.set_wallpaper(&device_name, &absolute_path) {
                    println!("Failed to set wallpaper for {} ({})", alias, device_name);
                    success = false;
                } else {
                    println!("Set wallpaper for {} ({})", alias, device_name);
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
            platform_config: self.platform_configs.clone(),
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

        // Try new format first
        if let Ok(config) = serde_json::from_str::<Config>(&contents) {
            self.platform_configs = config.platform_config;
            self.profiles = config.profiles;
            self.schedule = config.schedule;

            for (name, profile) in &mut self.profiles {
                profile.name = name.clone();
            }

            // Build alias list from current platform's monitor_map
            self.aliases = self.current_platform_config()
                .monitor_map
                .keys()
                .cloned()
                .collect();
            self.aliases.sort();

            for alias in &self.aliases {
                if let Some(device) = self.current_platform_config().monitor_map.get(alias) {
                    if !self.monitors.iter().any(|m| m.device_name == *device) {
                        println!("Warning: alias '{}' maps to device '{}' which was not detected", alias, device);
                    }
                }
            }

            println!("Configuration loaded from {}", filename);
            return true;
        }

        // Fall back to legacy format
        match serde_json::from_str::<LegacyConfig>(&contents) {
            Ok(legacy) => {
                println!("Detected legacy config format. Please add platform_config section.");
                self.profiles = legacy.profiles;
                self.schedule = legacy.schedule;

                for (name, profile) in &mut self.profiles {
                    profile.name = name.clone();
                }

                // Use device names as aliases for backward compatibility
                let mut device_names: Vec<String> = self.profiles.values()
                    .flat_map(|p| p.monitor_wallpapers.keys().cloned())
                    .collect();
                device_names.sort();
                device_names.dedup();

                let mut identity_map = HashMap::new();
                for name in &device_names {
                    identity_map.insert(name.clone(), name.clone());
                }

                #[cfg(target_os = "linux")]
                {
                    self.platform_configs.linux.monitor_map = identity_map;
                }
                #[cfg(windows)]
                {
                    self.platform_configs.windows.monitor_map = identity_map;
                }

                self.aliases = device_names;
                true
            }
            Err(e) => {
                println!("Failed to parse config from {}: {}", filename, e);
                false
            }
        }
    }
}
