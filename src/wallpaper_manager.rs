use std::collections::HashMap;
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::ops::Index;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use chrono::{Local, Timelike};
use winapi::um::winuser::MONITORINFOF_PRIMARY;
use windows::core::{BOOL, GUID, HRESULT, HSTRING, Result, PWSTR};

use windows::Win32::UI::Shell::{IDesktopWallpaper, DesktopWallpaper};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW};
use windows::Win32::Foundation::{RECT, LPARAM, FALSE, TRUE};
use windows::Win32::System::Com::{CoCreateInstance, CoInitialize, CoTaskMemFree, CoUninitialize, CLSCTX_ALL};

// Desktop wallpaper position constants
#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum DesktopWallpaperPosition {
    Center = 0,
    Tile = 1,
    Stretch = 2,
    Fit = 3,
    Fill = 4,
    Span = 5,
}

impl DesktopWallpaperPosition {
    fn to_string(&self) -> &'static str {
        match self {
            Self::Center => "Center",
            Self::Tile => "Tile",
            Self::Stretch => "Stretch",
            Self::Fit => "Fit",
            Self::Fill => "Fill",
            Self::Span => "Span",
        }
    }
}

#[derive(Clone)]
pub struct MonitorInfo {
    pub handle: HMONITOR,
    pub rect: RECT,
    pub device_name: String,
    pub is_primary: bool,
}

impl std::fmt::Debug for MonitorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonitorInfo")
            .field("handle", &(self.handle.0 as usize))
            .field("device_name", &self.device_name)
            .field("rect", &format!("({}, {}, {}, {})",
                                    self.rect.left, self.rect.top, self.rect.right, self.rect.bottom))
            .field("is_primary", &self.is_primary)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct WallpaperProfile {
    pub name: String,
    pub monitor_wallpapers: HashMap<String, String>, // deviceName -> wallpaperPath
}

#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    pub profile_name: String,
    pub hour: u32,
    pub minute: u32,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct WallpaperManager {
    pub monitors: Vec<MonitorInfo>,
    pub profiles: HashMap<String, WallpaperProfile>,
    pub schedule: Vec<ScheduleEntry>,
    pub scheduler_running: Arc<AtomicBool>,
}

// Helper functions for Windows API
fn wide_string_from_str(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn string_from_wide_ptr(ptr: *mut u16) -> String {
    if ptr.is_null() {
        return String::new();
    }

    unsafe {
        let mut len = 0;
        let mut temp_ptr = ptr;
        while *temp_ptr != 0 {
            len += 1;
            temp_ptr = temp_ptr.add(1);
        }

        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16_lossy(slice)
    }
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc_monitor: HDC,
    _lprc_monitor: *mut RECT,
    dwdata: LPARAM,
) -> BOOL {
    let monitors = &mut *(dwdata.0 as *mut Vec<MonitorInfo>);

    let mut mi: MONITORINFOEXW = std::mem::zeroed();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(hmonitor, &mut mi as *mut _ as *mut _) != FALSE {
        let device_name = String::from_utf16_lossy(&mi.szDevice)
            .trim_end_matches('\0')
            .to_string();

        let monitor_info = MonitorInfo {
            handle: hmonitor,
            rect: mi.monitorInfo.rcMonitor,
            device_name,
            is_primary: (mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
        };

        monitors.push(monitor_info);
    }

    TRUE
}

impl WallpaperManager {
    pub fn new() -> Self {
        let mut manager = Self {
            monitors: Vec::new(),
            profiles: HashMap::new(),
            schedule: Vec::new(),
            scheduler_running: Arc::new(AtomicBool::new(false)),
        };
        manager.refresh_monitors();
        manager
    }

    fn refresh_monitors(&mut self) {
        self.monitors.clear();

        unsafe {
            EnumDisplayMonitors(
                Option::None,
                Option::None,
                Some(monitor_enum_proc),
                LPARAM(&self.monitors as *const _ as isize),
            );
        }

        let wallpaper_monitor_ids = self.get_desktop_wallpaper_monitor_ids();

        for (i, (display_name, wallpaper_monitor_id)) in wallpaper_monitor_ids.iter().enumerate()
        {
            self.monitors[i].device_name = wallpaper_monitor_id.to_string(); //stupid hack
        }

        println!("\n=== Monitor Information ===");
        println!("EnumDisplayMonitors found {} monitors:", self.monitors.len());

        for (i, monitor) in self.monitors.iter().enumerate() {
            println!("  {}. {}{} - {}x{}",
                     i + 1,
                     monitor.device_name,
                     if monitor.is_primary { " (Primary)" } else { "" },
                     monitor.rect.right - monitor.rect.left,
                     monitor.rect.bottom - monitor.rect.top
            );
        }

        println!("\nIDesktopWallpaper found {} monitors:", wallpaper_monitor_ids.len());
        for (i, (display_name, wallpaper_monitor_id)) in wallpaper_monitor_ids.iter().enumerate() {
            println!("  {}. Display: {}", i + 1, display_name);
            println!("     Wallpaper ID: {}", wallpaper_monitor_id);

            let current_wallpaper = self.get_current_wallpaper_by_monitor_id(wallpaper_monitor_id);
            if !current_wallpaper.is_empty() {
                println!("     Current wallpaper: {}", current_wallpaper);
            }
        }
        println!("===========================\n");
    }

    pub fn get_current_wallpaper_by_monitor_id(&self, monitor_id: &str) -> String {
        let monitor_id_wide = HSTRING::from(monitor_id);

        unsafe {
            let hr_init = CoInitialize(Option::None);
            let com_initialized = hr_init == HRESULT(0); // S_OK

            let mut result = String::new();

            let hr: Result<IDesktopWallpaper> = CoCreateInstance(
                &DesktopWallpaper,
                None,
                CLSCTX_ALL,
            );

            if hr.is_ok() {
                let desktop = hr.unwrap();
                let hr = desktop.GetWallpaper(
                    &monitor_id_wide
                );

                if hr.is_ok() {
                    let ptr = hr.unwrap().0;
                    result = string_from_wide_ptr(ptr);
                    CoTaskMemFree(Some(ptr as _));
                }

            }

            if com_initialized {
                CoUninitialize();
            }

            result
        }
    }

    fn get_desktop_wallpaper_monitor_ids(&self) -> Vec<(String, String)> {
        let mut monitor_ids = Vec::new();

        unsafe {
            let hr_init = CoInitialize(None);
            let com_initialized = hr_init == HRESULT(0);

            let hr: Result<IDesktopWallpaper> = CoCreateInstance(
                &DesktopWallpaper,
                None,
                CLSCTX_ALL,
            );

            if hr.is_ok() {
                let wallpaper = hr.unwrap();
                let count_res: Result<u32> = wallpaper.GetMonitorDevicePathCount();
                if count_res.is_ok() {
                    let count = count_res.unwrap();
                    for i in 0..count {
                        let mut monitor_id_res: Result<PWSTR> = wallpaper.GetMonitorDevicePathAt(i);
                        if monitor_id_res.is_ok()
                        {
                            let str_ptr = monitor_id_res.unwrap();
                            let monitor_id_str = string_from_wide_ptr(str_ptr.0);

                            // Try to match with monitor list
                            let mut display_name = format!("Monitor {}", i + 1);
                            for monitor in &self.monitors {
                                if let Some(device_part) = monitor.device_name.strip_prefix("\\\\.\\") {
                                    if monitor_id_str.contains(device_part) {
                                        display_name = monitor.device_name.clone();
                                        break;
                                    }
                                }
                            }

                            monitor_ids.push((display_name, monitor_id_str));
                            CoTaskMemFree(Some(str_ptr.0 as _));
                        }
                    }
                }
            }

            if com_initialized {
                CoUninitialize();
            }
        }

        monitor_ids
    }

    fn set_wallpaper_for_monitor(&self, device_name: &str, wallpaper_path: &str) -> bool {
        let wallpaper_path_wide = HSTRING::from(wallpaper_path);

        unsafe {
            let hr_init = CoInitialize(None);
            let com_initialized = hr_init == HRESULT(0);

            let hr: Result<IDesktopWallpaper> = CoCreateInstance(
                &DesktopWallpaper,
                None,
                CLSCTX_ALL,
            );

            let mut success = false;

            if hr.is_ok() {
                let wallpaper = hr.unwrap();
                // Method 1: Try to find the correct monitor ID
                let count_res: Result<u32> = wallpaper.GetMonitorDevicePathCount();
                if count_res.is_ok() {
                    let count = count_res.unwrap();

                    for i in 0..count {
                        let mut monitor_id_res: Result<PWSTR> = wallpaper.GetMonitorDevicePathAt(i);
                        if monitor_id_res.is_ok()
                        {
                            let str_ptr = monitor_id_res.unwrap();
                            let monitor_id_str = string_from_wide_ptr(str_ptr.0);

                            // Check if this monitor matches our device name
                            let is_match = monitor_id_str == device_name
                                || monitor_id_str.contains(device_name)
                                || device_name.contains(&monitor_id_str)
                                || i == 0; // fallback to first monitor

                            if is_match {
                                println!("Trying to set wallpaper for monitor: {}", monitor_id_str);
                                let hr = wallpaper.SetWallpaper(
                                    str_ptr,
                                    &wallpaper_path_wide,
                                );

                                match hr {
                                    Ok(_) => {
                                        println!("Successfully set wallpaper using monitor ID: {}", monitor_id_str);
                                        success = true;
                                        CoTaskMemFree(Some(str_ptr.0 as _));
                                        break;
                                    }
                                    Err(E) => {
                                        println!("Failed to set wallpaper, HRESULT: 0x{:X}", E.code().0);
                                    }
                                }
                            }

                            CoTaskMemFree(Some(str_ptr.0 as _));
                        }
                    }
                }

                // Method 2: Try using device name directly
                if !success {
                    let device_name_wide = HSTRING::from(device_name);
                    println!("Trying direct device name: {}", device_name);
                    let hr = wallpaper.SetWallpaper(
                        &device_name_wide,
                        &wallpaper_path_wide,
                    );

                    if hr.is_ok() {
                        println!("Successfully set wallpaper using direct device name");
                        success = true;
                    }
                }

                // Method 3: Try setting for all monitors (NULL parameter)
                // if !success {
                //     println!("Trying to set wallpaper for all monitors");
                //     let hr = wallpaper.SetWallpaper(
                //         std::ptr::null(),
                //         wallpaper_path_wide.as_ptr(),
                //     );
                //
                //     if hr == 0 {
                //         println!("Successfully set wallpaper for all monitors");
                //         success = true;
                //     }
                // }
            }

            if com_initialized {
                CoUninitialize();
            }

            success
        }
    }

    /*fn set_wallpaper_fallback(&self, wallpaper_path: &str) -> bool {
        let wallpaper_path_wide = wide_string_from_str(wallpaper_path);

        unsafe {
            let result = SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                wallpaper_path_wide.as_ptr() as *mut c_void,
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            );

            result != FALSE
        }
    }*/

    fn set_wallpaper_for_monitor_with_fallback(&self, device_name: &str, wallpaper_path: &str) -> bool {
        // First try the modern IDesktopWallpaper approach
        if self.set_wallpaper_for_monitor(device_name, wallpaper_path) {
            return true;
        }

        // If that fails, fall back to SystemParametersInfo
        println!("IDesktopWallpaper failed for {}, using fallback method", device_name);
        // self.set_wallpaper_fallback(wallpaper_path)
        return false;
    }

    pub fn print_monitors(&mut self) {
        self.refresh_monitors();

        let wallpaper_monitor_ids = self.get_desktop_wallpaper_monitor_ids();

        println!("Available monitors for wallpaper setting:");
        println!("==========================================");

        if wallpaper_monitor_ids.is_empty() {
            println!("No monitors found via IDesktopWallpaper interface.");
            println!("Using fallback EnumDisplayMonitors data:");

            for (i, monitor) in self.monitors.iter().enumerate() {
                println!("{}. {}{} - {}x{}",
                         i + 1,
                         monitor.device_name,
                         if monitor.is_primary { " (Primary)" } else { "" },
                         monitor.rect.right - monitor.rect.left,
                         monitor.rect.bottom - monitor.rect.top
                );
                println!("   Use device name: {}\n", monitor.device_name);
            }
        } else {
            for (i, (display_name, wallpaper_monitor_id)) in wallpaper_monitor_ids.iter().enumerate() {
                // Find corresponding monitor info
                let monitor_info = self.monitors.iter().find(|monitor| {
                    if let Some(device_part) = monitor.device_name.strip_prefix("\\\\.\\") {
                        wallpaper_monitor_id.contains(device_part)
                    } else {
                        false
                    }
                });

                print!("{}. ", i + 1);
                if let Some(info) = monitor_info {
                    println!("{}{} - {}x{}",
                             info.device_name,
                             if info.is_primary { " (Primary)" } else { "" },
                             info.rect.right - info.rect.left,
                             info.rect.bottom - info.rect.top
                    );
                } else {
                    println!("Monitor {}", i + 1);
                }

                println!("   Use device name: {}", wallpaper_monitor_id);

                let current_wallpaper = self.get_current_wallpaper_by_monitor_id(wallpaper_monitor_id);
                if !current_wallpaper.is_empty() {
                    println!("   Current wallpaper: {}", current_wallpaper);
                }
                println!();
            }
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
                if !self.set_wallpaper_for_monitor_with_fallback(device_name, wallpaper_path) {
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
        let profiles = self.profiles.clone();

        thread::spawn(move || {
            while scheduler_running.load(Ordering::Relaxed) {
                let now = Local::now();
                let current_hour = now.hour();
                let current_minute = now.minute();

                for entry in &schedule {
                    if entry.enabled && entry.hour == current_hour && entry.minute == current_minute {
                        // Apply profile logic would go here
                        // We'd need to pass back to the main manager somehow
                        println!("Time to apply profile: {}", entry.profile_name);
                        // Sleep for a minute to avoid reapplying
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

    pub fn save_config(&self, filename: &str) -> bool {
        match std::fs::File::create(filename) {
            Ok(mut file) => {
                // Save profiles
                if writeln!(file, "[PROFILES]").is_err() {
                    println!("Failed to write to config file");
                    return false;
                }

                for (name, profile) in &self.profiles {
                    if writeln!(file, "PROFILE:{}", name).is_err() {
                        println!("Failed to write profile to config file");
                        return false;
                    }
                    for (device, wallpaper) in &profile.monitor_wallpapers {
                        if writeln!(file, "  {}={}", device, wallpaper).is_err() {
                            println!("Failed to write wallpaper mapping to config file");
                            return false;
                        }
                    }
                }

                // Save schedule
                if writeln!(file, "[SCHEDULE]").is_err() {
                    println!("Failed to write schedule section to config file");
                    return false;
                }

                for entry in &self.schedule {
                    if writeln!(file, "{},{},{},{}",
                                entry.profile_name,
                                entry.hour,
                                entry.minute,
                                if entry.enabled { 1 } else { 0 }
                    ).is_err() {
                        println!("Failed to write schedule entry to config file");
                        return false;
                    }
                }

                println!("Configuration saved to {}", filename);
                true
            }
            Err(e) => {
                println!("Failed to save config to {}: {}", filename, e);
                false
            }
        }
    }

    pub fn load_config(&mut self, filename: &str) -> bool {
        let file = match File::open(filename) {
            Ok(file) => file,
            Err(_) => {
                println!("Config file not found: {}", filename);
                return false;
            }
        };

        self.profiles.clear();
        self.schedule.clear();

        let reader = BufReader::new(file);
        let mut current_section = String::new();
        let mut current_profile = String::new();

        for line in reader.lines() {
            let line = match line {
                Ok(line) => line.to_string(),
                Err(_) => continue,
            };

            if line.is_empty() {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len()-1].to_string();
                continue;
            }

            match current_section.as_str() {
                "PROFILES" => {
                    if line.starts_with("PROFILE:") {
                        current_profile = line[8..].to_string();
                        self.profiles.insert(current_profile.clone(), WallpaperProfile {
                            name: current_profile.clone(),
                            monitor_wallpapers: HashMap::new(),
                        });
                    } else if line.starts_with("  ") && !current_profile.is_empty() {
                        if let Some(eq_pos) = line.find('=') {
                            let device = line[2..eq_pos].to_string();
                            let wallpaper = line[eq_pos + 1..].to_string();
                            if let Some(profile) = self.profiles.get_mut(&current_profile) {
                                profile.monitor_wallpapers.insert(device, wallpaper);
                            }
                        }
                    }
                }
                "SCHEDULE" => {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() == 4 {
                        if let (Ok(hour), Ok(minute), Ok(enabled_int)) = (
                            parts[1].parse::<u32>(),
                            parts[2].parse::<u32>(),
                            parts[3].parse::<i32>()
                        ) {
                            self.schedule.push(ScheduleEntry {
                                profile_name: parts[0].to_string(),
                                hour,
                                minute,
                                enabled: enabled_int == 1,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        println!("Configuration loaded from {}", filename);
        true
    }
}