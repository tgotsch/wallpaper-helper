# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build          # Debug build
cargo build --release # Release build
cargo run            # Run the app (launches GTK4 GUI)
cargo test --verbose # Run tests (no tests exist yet)
```

CI runs `cargo build --verbose && cargo test --verbose` on Ubuntu (see `.github/workflows/rust.yml`). Note: the app uses Windows-only APIs (`winapi`, `windows` crate) so CI builds will fail on Linux — the project is Windows-only.

## Architecture

This is a **Windows desktop wallpaper manager** with a GTK4 GUI. It lets users create named wallpaper profiles that map per-monitor wallpaper images, then apply them.

### Core modules

- **`src/main.rs`** — GTK4 application entry point and UI. Contains `WallpaperData` (per-monitor image widget with file-picker click handler) and `build_ui` which constructs the window with a profile selector dropdown, monitor image grid, apply button, and new-profile dialog. A commented-out CLI interface exists at the bottom of `main()`.

- **`src/wallpaper_manager.rs`** — Core logic. `WallpaperManager` holds:
  - `monitors: Vec<MonitorInfo>` — detected displays (populated via `EnumDisplayMonitors` + `IDesktopWallpaper`)
  - `profiles: HashMap<String, WallpaperProfile>` — named profiles mapping monitor device paths to wallpaper file paths
  - `schedule: Vec<ScheduleEntry>` — time-based profile switching (scheduler runs in a background thread)
  - Wallpaper setting uses the COM `IDesktopWallpaper` interface with a fallback strategy
  - Config persistence via JSON (`config.json`) using `serde`/`serde_json`

- **`src/wallpaper_source.rs`** — WIP/incomplete trait-based abstraction for wallpaper sources (`LocalFileSource`, `UrlSource`, `FolderSource`, `BooruSource`). Currently commented out of `main.rs`.

### Key patterns

- Monitor identification uses Windows `IDesktopWallpaper` device path strings (e.g., `\\?\DISPLAY#AOC2490#...`), not display names like `\\.\DISPLAY1`.
- COM is initialized/uninitialized per-call in `WallpaperManager` methods — each method that touches `IDesktopWallpaper` does its own `CoInitialize`/`CoUninitialize`.
- The GUI uses `Rc<RefCell<>>` for shared mutable state between GTK4 signal handlers.

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
