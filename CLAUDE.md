# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build          # Debug build
cargo build --release # Release build
cargo run            # Run the app (launches GTK4 GUI)
cargo test --verbose # Run tests (no tests exist yet)
```

CI runs `cargo build --verbose && cargo test --verbose` on both Ubuntu and Windows (see `.github/workflows/rust.yml`).

### Linux requirements

On Linux (KDE Plasma 6), ensure these are available:
- `kscreen-doctor` — for monitor detection (part of `kscreen` / `libkscreen`); optional, sysfs fallback exists
- `qdbus` or `qdbus6` — for reading/setting wallpapers via Plasma's scripting interface
- GTK4 dev libraries (`libgtk-4-dev` on Debian/Ubuntu)

## Architecture

This is a **cross-platform desktop wallpaper manager** with a GTK4 GUI. It lets users create named wallpaper profiles that map per-monitor wallpaper images, then apply them. A single `config.json` can be shared across platforms (e.g., on a network drive) using user-defined monitor aliases and per-platform settings.

### Platform backends

Platform-specific code lives behind a `WallpaperBackend` trait in `src/backend/`:

- **`src/backend/mod.rs`** — Defines the platform-agnostic `MonitorInfo` struct (`device_name`, `width`, `height`, `is_primary`), the `WallpaperBackend` trait (`refresh_monitors`, `get_current_wallpaper`, `set_wallpaper`), and a `create_backend()` factory using `#[cfg]` gates.
- **`src/backend/windows.rs`** (`#[cfg(windows)]`) — Windows backend using COM `IDesktopWallpaper` interface. Monitors detected via `EnumDisplayMonitors` + `IDesktopWallpaper` device paths.
- **`src/backend/kde.rs`** (`#[cfg(target_os = "linux")]`) — KDE Plasma 6 backend. Monitor detection via `kscreen-doctor --outputs` with sysfs fallback (`/sys/class/drm/card*-*`) when kscreen-doctor fails (common on Plasma 6). Wallpaper get/set via `qdbus`/`qdbus6` `evaluateScript` calls to `org.kde.plasmashell`, using Plasma 6's `currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General']` before `readConfig`/`writeConfig`. Maps connector names to Plasma screen indices via sequential assignment (alphabetical order). Note: Plasma's screen index ordering may not match connector name ordering — the config's `monitor_map` can compensate by mapping aliases to whichever connector produces the correct screen index.

### Core modules

- **`src/main.rs`** — GTK4 application entry point and UI. Contains `WallpaperData` (per-monitor `Picture` widget with file-picker click handler) and `build_ui` which constructs the window with a profile selector dropdown, monitor image grid, apply button, and new-profile dialog. Iterates aliases (not raw device names) from `get_alias_monitor_info()` to build the monitor grid.

- **`src/wallpaper_manager.rs`** — Platform-agnostic core logic. `WallpaperManager` holds:
  - `monitors: Vec<MonitorInfo>` — detected displays (delegated to backend)
  - `aliases: Vec<String>` — user-defined monitor names (e.g., "main", "left", "right"), derived from the current platform's `monitor_map` keys
  - `platform_configs: PlatformConfigs` — per-platform settings (base paths, monitor mappings)
  - `profiles: HashMap<String, WallpaperProfile>` — named profiles mapping aliases to relative wallpaper paths
  - `schedule: Vec<ScheduleEntry>` — time-based profile switching (scheduler runs in a background thread)
  - `backend: Box<dyn WallpaperBackend>` — platform-specific implementation
  - Config persistence via JSON (`config.json`) using `serde`/`serde_json`, with legacy format fallback

  Key types:
  - `PlatformConfig` — contains `wallpaper_base_path` (String) and `monitor_map` (HashMap\<String, MonitorMapping\>)
  - `MonitorMapping` — serde untagged enum: `Simple(String)` for just a device name, or `Detailed { device, width?, height? }` for device name with optional resolution override (used when the config maps aliases to "wrong" connectors as a Plasma workaround)
  - `PlatformConfigs` — holds `windows` and `linux` `PlatformConfig` fields

  Key methods:
  - `resolve_alias_to_device()` — maps alias to platform-specific device name via `monitor_map`
  - `resolve_wallpaper_path()` / `make_relative_path()` — converts between relative (stored in profiles) and absolute paths using `wallpaper_base_path`
  - `get_alias_monitor_info()` — returns `(alias, Option<MonitorInfo>)` pairs, overriding resolution from `MonitorMapping::Detailed` when present
  - `apply_profile()` — resolves aliases to device names and relative to absolute paths, then delegates to backend

- **`src/wallpaper_source.rs`** — WIP/incomplete trait-based abstraction for wallpaper sources. Currently commented out.

### Key patterns

- Profiles use **aliases** (e.g., "main", "left", "right") as monitor keys and **relative paths** for wallpapers. Platform-specific device names and base paths are in `platform_config`.
- The GUI uses `gtk::Picture` (not `Image`) for wallpaper thumbnails so they scale to fit. Uses `Rc<RefCell<>>` for shared mutable state between GTK4 signal handlers.
- `WallpaperManager` is not `Clone` — it is always behind `Rc<RefCell<>>`.
- Config loading tries the new `platform_config` format first, then falls back to a legacy format (profiles with raw device names, no platform_config section).

### Config file format (`config.json`)

JSON file with `platform_config`, `profiles`, and `schedule` top-level keys, serialized via `serde`. Profiles use aliases and relative wallpaper paths. Each platform section maps aliases to platform-specific device names and provides the wallpaper base path.

```json
{
  "platform_config": {
    "windows": {
      "wallpaper_base_path": "Y:\\wallpapers",
      "monitor_map": {
        "main": "\\\\?\\DISPLAY#GSM5C34#...",
        "left": "\\\\?\\DISPLAY#AOC2490#...",
        "right": "\\\\?\\DISPLAY#AOC2590#..."
      }
    },
    "linux": {
      "wallpaper_base_path": "/home/user/wallpapers",
      "monitor_map": {
        "main": {"device": "DP-2", "width": 2560, "height": 1440},
        "left": "DP-3",
        "right": "HDMI-A-1"
      }
    }
  },
  "profiles": {
    "profile_name": {
      "monitor_wallpapers": {
        "main": "relative/path/to/wallpaper.png",
        "left": "another_wallpaper.png"
      },
      "tags": ["tag1", "tag2"]
    }
  },
  "schedule": [
    {
      "profile_name": "profile_name",
      "hour": 8,
      "minute": 0,
      "enabled": true
    }
  ]
}
```

Monitor map values can be either a plain string (device name only) or an object with `device`, `width`, and `height` fields. The detailed form is useful when the config maps an alias to a different connector than the physical monitor (e.g., as a Plasma screen index workaround) and the resolution display in the GUI would otherwise be incorrect.
