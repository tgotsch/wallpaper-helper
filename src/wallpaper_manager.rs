use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use chrono::{Local, Timelike};
use log::{info, warn, error};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCollection {
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MonitorMapping {
    Simple(String),
    Detailed {
        device: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
    },
}

impl MonitorMapping {
    pub fn device(&self) -> &str {
        match self {
            MonitorMapping::Simple(s) => s,
            MonitorMapping::Detailed { device, .. } => device,
        }
    }

    pub fn resolution(&self) -> Option<(u32, u32)> {
        match self {
            MonitorMapping::Simple(_) => None,
            MonitorMapping::Detailed { width, height, .. } => {
                match (width, height) {
                    (Some(w), Some(h)) => Some((*w, *h)),
                    _ => None,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub wallpaper_base_path: String,
    pub monitor_map: HashMap<String, MonitorMapping>, // alias -> platform-specific device name (+ optional resolution)
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
    #[serde(default)]
    collections: HashMap<String, ProfileCollection>,
    schedule: Vec<ScheduleEntry>,
}

#[derive(Debug, Deserialize)]
struct LegacyConfig {
    profiles: HashMap<String, WallpaperProfile>,
    schedule: Vec<ScheduleEntry>,
}

pub struct ProfileMismatchInfo {
    pub extra_aliases: Vec<String>,
    pub missing_aliases: Vec<String>,
}

impl ProfileMismatchInfo {
    pub fn has_warnings(&self) -> bool {
        !self.extra_aliases.is_empty() || !self.missing_aliases.is_empty()
    }
}

pub struct WallpaperManager {
    pub monitors: Vec<MonitorInfo>,
    pub aliases: Vec<String>,
    pub platform_configs: PlatformConfigs,
    pub profiles: HashMap<String, WallpaperProfile>,
    pub collections: HashMap<String, ProfileCollection>,
    pub collection_cycle_indices: HashMap<String, usize>,
    pub schedule: Vec<ScheduleEntry>,
    last_schedule_minute: Option<(u32, u32)>,
    backend: Box<dyn WallpaperBackend>,
}

impl WallpaperManager {
    pub fn new() -> Self {
        let mut backend = create_backend();
        let monitors = backend.refresh_monitors();

        info!("=== Monitor Information ===");
        info!("Found {} monitors:", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            info!("  {}. {}{} - {}x{}",
                     i + 1,
                     monitor.device_name,
                     if monitor.is_primary { " (Primary)" } else { "" },
                     monitor.width,
                     monitor.height,
            );
        }
        info!("===========================");

        Self {
            monitors,
            aliases: Vec::new(),
            platform_configs: PlatformConfigs::default(),
            profiles: HashMap::new(),
            collections: HashMap::new(),
            collection_cycle_indices: HashMap::new(),
            schedule: Vec::new(),
            last_schedule_minute: None,
            backend,
        }
    }

    fn current_platform_config(&self) -> &PlatformConfig {
        #[cfg(windows)]
        { &self.platform_configs.windows }
        #[cfg(target_os = "linux")]
        { &self.platform_configs.linux }
    }

    pub fn current_platform_config_mut(&mut self) -> &mut PlatformConfig {
        #[cfg(windows)]
        { &mut self.platform_configs.windows }
        #[cfg(target_os = "linux")]
        { &mut self.platform_configs.linux }
    }

    /// Re-derive the alias list from the current platform's monitor_map. Call
    /// after mutating the monitor map through `current_platform_config_mut()`.
    pub fn sync_aliases(&mut self) {
        self.aliases = self.current_platform_config()
            .monitor_map
            .keys()
            .cloned()
            .collect();
        self.aliases.sort();
    }

    fn resolve_alias_to_device(&self, alias: &str) -> Option<&str> {
        self.current_platform_config().monitor_map.get(alias).map(|m| m.device())
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
            let mapping = self.current_platform_config().monitor_map.get(alias);
            let mut monitor_info = mapping
                .map(|m| m.device())
                .and_then(|dev| self.monitors.iter().find(|m| m.device_name == dev))
                .cloned();

            // Override resolution from config if specified
            if let (Some(ref mut info), Some(mapping)) = (&mut monitor_info, mapping) {
                if let Some((w, h)) = mapping.resolution() {
                    info.width = w;
                    info.height = h;
                }
            }

            (alias.clone(), monitor_info)
        }).collect()
    }

    pub fn check_profile_mismatch(&self, profile_name: &str) -> Option<ProfileMismatchInfo> {
        let profile = self.profiles.get(profile_name)?;
        let system_aliases: HashSet<&String> = self.aliases.iter().collect();
        let profile_aliases: HashSet<&String> = profile.monitor_wallpapers.keys().collect();

        let extra: Vec<String> = profile_aliases.difference(&system_aliases)
            .map(|s| (*s).clone())
            .collect();
        let missing: Vec<String> = system_aliases.difference(&profile_aliases)
            .map(|s| (*s).clone())
            .collect();

        let mut info = ProfileMismatchInfo {
            extra_aliases: extra,
            missing_aliases: missing,
        };
        info.extra_aliases.sort();
        info.missing_aliases.sort();
        Some(info)
    }

    pub fn refresh_monitors(&mut self) {
        self.monitors = self.backend.refresh_monitors();

        info!("=== Monitor Information ===");
        info!("Found {} monitors:", self.monitors.len());
        for (i, monitor) in self.monitors.iter().enumerate() {
            info!("  {}. {}{} - {}x{}",
                     i + 1,
                     monitor.device_name,
                     if monitor.is_primary { " (Primary)" } else { "" },
                     monitor.width,
                     monitor.height,
            );
        }
        info!("===========================");
    }

    pub fn get_current_wallpaper_by_alias(&self, alias: &str) -> String {
        match self.resolve_alias_to_device(alias) {
            Some(device) => self.backend.get_current_wallpaper(device),
            None => {
                warn!("No device mapping found for alias '{}'", alias);
                String::new()
            }
        }
    }

    pub fn print_monitors(&mut self) {
        self.refresh_monitors();

        info!("Available monitors for wallpaper setting:");
        info!("==========================================");

        for (i, monitor) in self.monitors.iter().enumerate() {
            info!("{}. {}{} - {}x{}",
                     i + 1,
                     monitor.device_name,
                     if monitor.is_primary { " (Primary)" } else { "" },
                     monitor.width,
                     monitor.height,
            );
            info!("   Use device name: {}", monitor.device_name);

            let current_wallpaper = self.backend.get_current_wallpaper(&monitor.device_name);
            if !current_wallpaper.is_empty() {
                info!("   Current wallpaper: {}", current_wallpaper);
            }
        }

        info!("==========================================");
        info!("Tip: Copy the 'Use device name' exactly when setting wallpapers.");
    }

    pub fn create_profile(&mut self, profile_name: &str) -> bool {
        if self.profiles.contains_key(profile_name) {
            warn!("Profile '{}' already exists!", profile_name);
            return false;
        }

        self.profiles.insert(profile_name.to_string(), WallpaperProfile {
            name: profile_name.to_string(),
            monitor_wallpapers: HashMap::new(),
            tags: Vec::new(),
        });

        info!("Profile '{}' created.", profile_name);
        true
    }

    pub fn delete_profile(&mut self, profile_name: &str) -> bool {
        if self.profiles.remove(profile_name).is_none() {
            warn!("Profile '{}' does not exist!", profile_name);
            return false;
        }
        info!("Profile '{}' deleted.", profile_name);
        true
    }

    pub fn set_wallpaper_in_profile(&mut self, profile_name: &str, alias: &str, wallpaper_path: &str) -> bool {
        if !self.profiles.contains_key(profile_name) {
            warn!("Profile '{}' not found!", profile_name);
            return false;
        }

        // Verify file exists (absolute path from file chooser)
        if !Path::new(wallpaper_path).exists() {
            warn!("Wallpaper file not found: {}", wallpaper_path);
            return false;
        }

        // Check if it's a supported image format
        if let Some(extension) = Path::new(wallpaper_path).extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tiff") {
                warn!("Unsupported image format: {}", ext);
                warn!("Supported formats: jpg, jpeg, png, bmp, gif, tiff");
                return false;
            }
        }

        // Validate alias exists
        if !self.aliases.contains(&alias.to_string()) {
            warn!("Monitor alias '{}' not found!", alias);
            warn!("Available aliases:");
            for a in &self.aliases {
                warn!("  {}", a);
            }
            return false;
        }

        // Convert absolute path to relative for storage
        let relative_path = self.make_relative_path(wallpaper_path);

        if let Some(profile) = self.profiles.get_mut(profile_name) {
            profile.monitor_wallpapers.insert(alias.to_string(), relative_path);
            info!("Added wallpaper to profile '{}' for monitor {}", profile_name, alias);
            true
        } else {
            false
        }
    }

    pub fn apply_profile(&self, profile_name: &str) -> bool {
        if let Some(profile) = self.profiles.get(profile_name) {
            let mut success = true;
            info!("Applying profile '{}'...", profile_name);

            for (alias, relative_path) in &profile.monitor_wallpapers {
                let device_name = match self.resolve_alias_to_device(alias) {
                    Some(dev) => dev.to_string(),
                    None => {
                        warn!("No device mapping found for alias '{}' on this platform", alias);
                        success = false;
                        continue;
                    }
                };

                let absolute_path = self.resolve_wallpaper_path(relative_path);

                if !self.backend.set_wallpaper(&device_name, &absolute_path) {
                    error!("Failed to set wallpaper for {} ({})", alias, device_name);
                    success = false;
                } else {
                    info!("Set wallpaper for {} ({})", alias, device_name);
                }
            }

            success
        } else {
            warn!("Profile '{}' not found!", profile_name);
            false
        }
    }

    pub fn list_profiles(&self) -> Vec<String> {
        if self.profiles.is_empty() {
            info!("No profiles created.");
            return Vec::new();
        }

        info!("Available profiles:");
        let mut profile_names = Vec::new();
        for (name, profile) in &self.profiles {
            info!("- {} ({} monitors)", name, profile.monitor_wallpapers.len());
            profile_names.push(name.clone());
        }

        profile_names
    }

    pub fn add_schedule(&mut self, profile_name: &str, hour: u32, minute: u32) -> bool {
        if !self.profiles.contains_key(profile_name) {
            warn!("Profile '{}' not found!", profile_name);
            return false;
        }

        if hour > 23 || minute > 59 {
            warn!("Invalid time format. Use 24-hour format (0-23 for hours, 0-59 for minutes).");
            return false;
        }

        self.schedule.push(ScheduleEntry {
            profile_name: profile_name.to_string(),
            hour,
            minute,
            enabled: true,
        });

        info!("Scheduled profile '{}' at {:02}:{:02}", profile_name, hour, minute);
        true
    }

    pub fn list_schedule(&self) {
        if self.schedule.is_empty() {
            info!("No scheduled profiles.");
            return;
        }

        info!("Scheduled profiles:");
        for (i, entry) in self.schedule.iter().enumerate() {
            info!("{}. {} at {:02}:{:02}{}",
                     i + 1,
                     entry.profile_name,
                     entry.hour,
                     entry.minute,
                     if entry.enabled { " (enabled)" } else { " (disabled)" }
            );
        }
    }

    pub fn check_and_apply_schedule(&mut self) -> Option<String> {
        let now = Local::now();
        let current_hour = now.hour();
        let current_minute = now.minute();
        let current = (current_hour, current_minute);

        // Clear guard when minute changes
        if self.last_schedule_minute.is_some() && self.last_schedule_minute != Some(current) {
            self.last_schedule_minute = None;
        }

        // Don't re-trigger in the same minute
        if self.last_schedule_minute.is_some() {
            return None;
        }

        for entry in &self.schedule {
            if entry.enabled && entry.hour == current_hour && entry.minute == current_minute {
                let name = entry.profile_name.clone();
                self.last_schedule_minute = Some(current);
                self.apply_profile(&name);
                return Some(name);
            }
        }

        None
    }

    pub fn add_tag(&mut self, tag_name: &str, profile_name: &str) {
        if let Some(found) = self.profiles.get_mut(profile_name) {
            found.tags.push(tag_name.to_string());
        }
    }

    pub fn get_tags(&self, profile_name: &str) -> Vec<String> {
        self.profiles.values().filter(|profile| profile.name == profile_name).map(|profile| profile.tags.clone()).flatten().collect()
    }

    // --- Collection methods ---

    pub fn create_collection(&mut self, name: &str) -> bool {
        if self.collections.contains_key(name) {
            warn!("Collection '{}' already exists!", name);
            return false;
        }
        self.collections.insert(name.to_string(), ProfileCollection {
            profiles: Vec::new(),
        });
        info!("Collection '{}' created.", name);
        true
    }

    pub fn delete_collection(&mut self, name: &str) -> bool {
        if self.collections.remove(name).is_some() {
            self.collection_cycle_indices.remove(name);
            info!("Collection '{}' deleted.", name);
            true
        } else {
            warn!("Collection '{}' not found!", name);
            false
        }
    }

    pub fn add_profile_to_collection(&mut self, collection: &str, profile: &str) -> bool {
        if !self.profiles.contains_key(profile) {
            warn!("Profile '{}' not found!", profile);
            return false;
        }
        if let Some(col) = self.collections.get_mut(collection) {
            if col.profiles.contains(&profile.to_string()) {
                warn!("Profile '{}' already in collection '{}'", profile, collection);
                return false;
            }
            col.profiles.push(profile.to_string());
            true
        } else {
            warn!("Collection '{}' not found!", collection);
            false
        }
    }

    pub fn remove_profile_from_collection(&mut self, collection: &str, profile: &str) -> bool {
        if let Some(col) = self.collections.get_mut(collection) {
            let before = col.profiles.len();
            col.profiles.retain(|p| p != profile);
            col.profiles.len() < before
        } else {
            false
        }
    }

    pub fn get_valid_collection_profiles(&self, collection: &str) -> Vec<String> {
        match self.collections.get(collection) {
            Some(col) => col.profiles.iter()
                .filter(|p| self.profiles.contains_key(p.as_str()))
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn apply_random_from_collection(&mut self, collection: &str) -> Option<String> {
        use rand::seq::SliceRandom;
        let valid = self.get_valid_collection_profiles(collection);
        if valid.is_empty() {
            return None;
        }
        let mut rng = rand::thread_rng();
        let chosen = valid.choose(&mut rng)?.clone();
        self.apply_profile(&chosen);
        Some(chosen)
    }

    pub fn apply_next_in_collection(&mut self, collection: &str) -> Option<String> {
        let valid = self.get_valid_collection_profiles(collection);
        if valid.is_empty() {
            return None;
        }
        let idx = self.collection_cycle_indices.entry(collection.to_string()).or_insert(0);
        *idx = (*idx + 1) % valid.len();
        let chosen = valid[*idx].clone();
        self.apply_profile(&chosen);
        Some(chosen)
    }

    pub fn apply_prev_in_collection(&mut self, collection: &str) -> Option<String> {
        let valid = self.get_valid_collection_profiles(collection);
        if valid.is_empty() {
            return None;
        }
        let idx = self.collection_cycle_indices.entry(collection.to_string()).or_insert(0);
        *idx = if *idx == 0 { valid.len() - 1 } else { *idx - 1 };
        let chosen = valid[*idx].clone();
        self.apply_profile(&chosen);
        Some(chosen)
    }

    pub fn save_config(&self, filename: &str) -> bool {
        let config = Config {
            platform_config: self.platform_configs.clone(),
            profiles: self.profiles.clone(),
            collections: self.collections.clone(),
            schedule: self.schedule.clone(),
        };

        match serde_json::to_string_pretty(&config) {
            Ok(json) => {
                match fs::write(filename, json) {
                    Ok(_) => {
                        info!("Configuration saved to {}", filename);
                        true
                    }
                    Err(e) => {
                        error!("Failed to save config to {}: {}", filename, e);
                        false
                    }
                }
            }
            Err(e) => {
                error!("Failed to serialize config: {}", e);
                false
            }
        }
    }

    pub fn load_config(&mut self, filename: &str) -> bool {
        let contents = match fs::read_to_string(filename) {
            Ok(contents) => contents,
            Err(_) => {
                warn!("Config file not found: {}", filename);
                return false;
            }
        };

        // Try new format first
        if let Ok(config) = serde_json::from_str::<Config>(&contents) {
            self.platform_configs = config.platform_config;
            self.profiles = config.profiles;
            self.collections = config.collections;
            self.schedule = config.schedule;

            for (name, profile) in &mut self.profiles {
                profile.name = name.clone();
            }

            // Build alias list from current platform's monitor_map
            self.sync_aliases();

            for alias in &self.aliases {
                if let Some(mapping) = self.current_platform_config().monitor_map.get(alias) {
                    let device = mapping.device();
                    if !self.monitors.iter().any(|m| m.device_name == device) {
                        warn!("Alias '{}' maps to device '{}' which was not detected", alias, device);
                    }
                }
            }

            info!("Configuration loaded from {}", filename);
            return true;
        }

        // Fall back to legacy format
        match serde_json::from_str::<LegacyConfig>(&contents) {
            Ok(legacy) => {
                warn!("Detected legacy config format. Please add platform_config section.");
                self.profiles = legacy.profiles;
                self.collections = HashMap::new();
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
                    identity_map.insert(name.clone(), MonitorMapping::Simple(name.clone()));
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
                error!("Failed to parse config from {}: {}", filename, e);
                false
            }
        }
    }
}
