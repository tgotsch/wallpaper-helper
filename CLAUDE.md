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
- `kscreen-doctor` — for monitor detection (part of `kscreen` / `libkscreen`)
- `qdbus` or `qdbus6` — for reading/setting wallpapers via Plasma's scripting interface
- GTK4 dev libraries (`libgtk-4-dev` on Debian/Ubuntu)

## Architecture

This is a **cross-platform desktop wallpaper manager** with a GTK4 GUI. It lets users create named wallpaper profiles that map per-monitor wallpaper images, then apply them.

### Platform backends

Platform-specific code lives behind a `WallpaperBackend` trait in `src/backend/`:

- **`src/backend/mod.rs`** — Defines the platform-agnostic `MonitorInfo` struct (`device_name`, `width`, `height`, `is_primary`), the `WallpaperBackend` trait (`refresh_monitors`, `get_current_wallpaper`, `set_wallpaper`), and a `create_backend()` factory using `#[cfg]` gates.
- **`src/backend/windows.rs`** (`#[cfg(windows)]`) — Windows backend using COM `IDesktopWallpaper` interface. Monitors detected via `EnumDisplayMonitors` + `IDesktopWallpaper` device paths.
- **`src/backend/kde.rs`** (`#[cfg(target_os = "linux")]`) — KDE Plasma 6 backend. Monitor detection via `kscreen-doctor --outputs`. Wallpaper get/set via `qdbus` `evaluateScript` calls to `org.kde.plasmashell`. Maps connector names (e.g., `DP-1`) to Plasma screen indices by matching geometry positions.

### Core modules

- **`src/main.rs`** — GTK4 application entry point and UI. Contains `WallpaperData` (per-monitor image widget with file-picker click handler) and `build_ui` which constructs the window with a profile selector dropdown, monitor image grid, apply button, and new-profile dialog.

- **`src/wallpaper_manager.rs`** — Platform-agnostic core logic. `WallpaperManager` holds:
  - `monitors: Vec<MonitorInfo>` — detected displays (delegated to backend)
  - `profiles: HashMap<String, WallpaperProfile>` — named profiles mapping monitor device names to wallpaper file paths
  - `schedule: Vec<ScheduleEntry>` — time-based profile switching (scheduler runs in a background thread)
  - `backend: Box<dyn WallpaperBackend>` — platform-specific implementation
  - Config persistence via JSON (`config.json`) using `serde`/`serde_json`

- **`src/wallpaper_source.rs`** — WIP/incomplete trait-based abstraction for wallpaper sources. Currently commented out.

### Key patterns

- Monitor identification uses platform-specific strings: Windows `IDesktopWallpaper` device paths (e.g., `\\?\DISPLAY#AOC2490#...`) on Windows, connector names (e.g., `DP-1`, `HDMI-1`) on Linux. Config files are platform-specific.
- The GUI uses `Rc<RefCell<>>` for shared mutable state between GTK4 signal handlers.
- `WallpaperManager` is not `Clone` — it is always behind `Rc<RefCell<>>`.

### Config file format (`config.json`)

JSON file with `profiles` and `schedule` top-level keys, serialized via `serde`. Example:

```json
{
  "profiles": {
    "profile_name": {
      "monitor_wallpapers": {
        "\\\\?\\DISPLAY#...": "C:\\path\\to\\wallpaper.png"
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
