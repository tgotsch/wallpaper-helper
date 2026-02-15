#![cfg(windows)]

use super::{MonitorInfo, WallpaperBackend};

use log::{info, error};
use winapi::um::winuser::MONITORINFOF_PRIMARY;
use windows::core::{BOOL, HRESULT, HSTRING, Result, PWSTR};
use windows::Win32::UI::Shell::{IDesktopWallpaper, DesktopWallpaper};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW};
use windows::Win32::Foundation::{RECT, LPARAM, FALSE, TRUE};
use windows::Win32::System::Com::{CoCreateInstance, CoInitialize, CoTaskMemFree, CoUninitialize, CLSCTX_ALL};

struct RawMonitorInfo {
    _handle: HMONITOR,
    rect: RECT,
    device_name: String,
    is_primary: bool,
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
    let monitors = &mut *(dwdata.0 as *mut Vec<RawMonitorInfo>);

    let mut mi: MONITORINFOEXW = std::mem::zeroed();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(hmonitor, &mut mi as *mut _ as *mut _) != FALSE {
        let device_name = String::from_utf16_lossy(&mi.szDevice)
            .trim_end_matches('\0')
            .to_string();

        monitors.push(RawMonitorInfo {
            _handle: hmonitor,
            rect: mi.monitorInfo.rcMonitor,
            device_name,
            is_primary: (mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
        });
    }

    TRUE
}

pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Self {
        WindowsBackend
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
                        let monitor_id_res: Result<PWSTR> = wallpaper.GetMonitorDevicePathAt(i);
                        if monitor_id_res.is_ok() {
                            let str_ptr = monitor_id_res.unwrap();
                            let monitor_id_str = string_from_wide_ptr(str_ptr.0);

                            let display_name = format!("Monitor {}", i + 1);

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
                let count_res: Result<u32> = wallpaper.GetMonitorDevicePathCount();
                if count_res.is_ok() {
                    let count = count_res.unwrap();

                    for i in 0..count {
                        let monitor_id_res: Result<PWSTR> = wallpaper.GetMonitorDevicePathAt(i);
                        if monitor_id_res.is_ok() {
                            let str_ptr = monitor_id_res.unwrap();
                            let monitor_id_str = string_from_wide_ptr(str_ptr.0);

                            let is_match = monitor_id_str == device_name
                                || monitor_id_str.contains(device_name)
                                || device_name.contains(&monitor_id_str);

                            if is_match {
                                info!("Trying to set wallpaper for monitor: {}", monitor_id_str);
                                let hr = wallpaper.SetWallpaper(
                                    str_ptr,
                                    &wallpaper_path_wide,
                                );

                                match hr {
                                    Ok(_) => {
                                        info!("Successfully set wallpaper using monitor ID: {}", monitor_id_str);
                                        success = true;
                                        CoTaskMemFree(Some(str_ptr.0 as _));
                                        break;
                                    }
                                    Err(e) => {
                                        error!("Failed to set wallpaper, HRESULT: 0x{:X}", e.code().0);
                                    }
                                }
                            }

                            CoTaskMemFree(Some(str_ptr.0 as _));
                        }
                    }
                }

                if !success {
                    let device_name_wide = HSTRING::from(device_name);
                    info!("Trying direct device name: {}", device_name);
                    let hr = wallpaper.SetWallpaper(
                        &device_name_wide,
                        &wallpaper_path_wide,
                    );

                    if hr.is_ok() {
                        info!("Successfully set wallpaper using direct device name");
                        success = true;
                    }
                }
            }

            if com_initialized {
                CoUninitialize();
            }

            success
        }
    }
}

impl WallpaperBackend for WindowsBackend {
    fn refresh_monitors(&mut self) -> Vec<MonitorInfo> {
        let mut raw_monitors: Vec<RawMonitorInfo> = Vec::new();

        unsafe {
            let _ = EnumDisplayMonitors(
                Option::None,
                Option::None,
                Some(monitor_enum_proc),
                LPARAM(&mut raw_monitors as *mut _ as isize),
            );
        }

        let wallpaper_ids = self.get_desktop_wallpaper_monitor_ids();

        let mut monitors: Vec<MonitorInfo> = Vec::new();

        for (i, raw) in raw_monitors.iter().enumerate() {
            let device_name = if i < wallpaper_ids.len() {
                wallpaper_ids[i].1.clone()
            } else {
                raw.device_name.clone()
            };

            monitors.push(MonitorInfo {
                device_name,
                width: (raw.rect.right - raw.rect.left) as u32,
                height: (raw.rect.bottom - raw.rect.top) as u32,
                is_primary: raw.is_primary,
            });
        }

        monitors
    }

    fn get_current_wallpaper(&self, monitor_id: &str) -> String {
        let monitor_id_wide = HSTRING::from(monitor_id);

        unsafe {
            let hr_init = CoInitialize(Option::None);
            let com_initialized = hr_init == HRESULT(0);

            let mut result = String::new();

            let hr: Result<IDesktopWallpaper> = CoCreateInstance(
                &DesktopWallpaper,
                None,
                CLSCTX_ALL,
            );

            if hr.is_ok() {
                let desktop = hr.unwrap();
                let hr = desktop.GetWallpaper(&monitor_id_wide);

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

    fn set_wallpaper(&self, monitor_id: &str, wallpaper_path: &str) -> bool {
        if self.set_wallpaper_for_monitor(monitor_id, wallpaper_path) {
            return true;
        }

        error!("IDesktopWallpaper failed for {}, fallback not available", monitor_id);
        false
    }
}
