#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod kde;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub device_name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

pub trait WallpaperBackend {
    fn refresh_monitors(&mut self) -> Vec<MonitorInfo>;
    fn get_current_wallpaper(&self, monitor_id: &str) -> String;
    fn set_wallpaper(&self, monitor_id: &str, wallpaper_path: &str) -> bool;
}

pub fn create_backend() -> Box<dyn WallpaperBackend> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsBackend::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(kde::KdeBackend::new())
    }
}
