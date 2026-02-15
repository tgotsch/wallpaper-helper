# WallpaperHelper_rs

A cross-platform desktop wallpaper manager with a GTK4 GUI. Create named wallpaper profiles that map per-monitor images, then apply them with a click. A single config file can be shared across Windows and Linux (e.g., on a network drive) using user-defined monitor aliases and per-platform settings.

## Features

- **Multi-monitor profiles** — Assign a wallpaper to each monitor by alias (e.g., "main", "left", "right") and save as a named profile.
- **Collections** — Group profiles into collections and cycle through them with Prev/Next/Random buttons or an automatic slideshow timer.
- **Scheduler** — Schedule profiles to apply at specific times of day.
- **System tray** — Closing the window minimizes to the system tray. Slideshows and the scheduler continue running in the background. Right-click the tray icon for "Show Window" or "Quit". On Windows, double-clicking the tray icon also restores the window.
- **Single instance** — Launching the app again re-shows the existing window instead of starting a second process.
- **Cross-platform config** — One `config.json` with per-platform sections for device mappings and base paths.

## Supported Platforms

- **Windows** — Uses the COM `IDesktopWallpaper` interface for per-monitor wallpaper control. Tray icon via the `tray-icon` crate (native Win32).
- **Linux (KDE Plasma 6)** — Uses `qdbus`/`qdbus6` to script `org.kde.plasmashell` for wallpaper get/set. Monitor detection via `kscreen-doctor` with a sysfs fallback. Tray icon via the `ksni` crate (StatusNotifierItem over D-Bus).

## Build & Run

### Requirements

- Rust toolchain (stable)
- GTK4 development libraries

#### Linux (KDE Plasma 6)

- `libgtk-4-dev` (Debian/Ubuntu) or equivalent
- `kscreen-doctor` (optional, for monitor detection; sysfs fallback exists)
- `qdbus` or `qdbus6` (for reading/setting wallpapers)

### Building

```bash
cargo build            # Debug build
cargo build --release  # Release build
cargo run              # Run the app
cargo test --verbose   # Run tests
```

## Usage

1. **Set up `config.json`** — Define your platform config with monitor aliases, device mappings, and wallpaper base path. See [Config format](#config-format) below.
2. **Launch the app** — The window shows your monitors with current wallpapers. Select a profile from the dropdown to preview it, click "Apply" to set it.
3. **Create profiles** — Click "New profile" to save the current wallpaper assignments as a named profile. Click a monitor image to change its wallpaper via file picker.
4. **Collections** — Click "Collections..." to create/manage collections of profiles. Select a collection from the dropdown to access Prev/Next/Random buttons and the slideshow timer.
5. **Close to tray** — Closing the window hides it to the system tray. The app continues running (slideshows, scheduler). Use the tray menu to restore the window or quit.

## Config Format

`config.json` is a JSON file with `platform_config`, `profiles`, `collections`, and `schedule` sections:

```json
{
  "platform_config": {
    "windows": {
      "wallpaper_base_path": "Y:\\wallpapers",
      "monitor_map": {
        "main": "\\\\?\\DISPLAY#GSM5C34#...",
        "left": "\\\\?\\DISPLAY#AOC2490#..."
      }
    },
    "linux": {
      "wallpaper_base_path": "/home/user/wallpapers",
      "monitor_map": {
        "main": {"device": "DP-2", "width": 2560, "height": 1440},
        "left": "DP-3"
      }
    }
  },
  "profiles": {
    "default": {
      "monitor_wallpapers": {
        "main": "relative/path/to/wallpaper.png",
        "left": "another_wallpaper.png"
      },
      "tags": []
    }
  },
  "collections": {
    "favorites": {
      "profiles": ["default", "evening"]
    }
  },
  "schedule": [
    {
      "profile_name": "default",
      "hour": 8,
      "minute": 0,
      "enabled": true
    }
  ]
}
```

- **Monitor map values** can be a plain string (device name) or an object with `device`, `width`, and `height` fields. The detailed form is useful when the alias maps to a different connector than the physical monitor (e.g., as a Plasma screen index workaround).
- **Profiles** use aliases as keys and relative wallpaper paths (relative to `wallpaper_base_path`).
- **Schedule entries** use 24-hour time. The scheduler checks every 30 seconds and applies the first matching enabled entry.
