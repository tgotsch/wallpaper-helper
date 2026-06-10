# Wallpaper Helper

A cross-platform desktop wallpaper manager with a [Dioxus](https://dioxuslabs.com/) (webview) GUI. Create named wallpaper profiles that map per-monitor images, then apply them with one click. A single config file can be shared across platforms (e.g., on a network drive) using user-defined monitor aliases and per-platform settings.

## Features

- **Per-monitor wallpaper control** — assign different wallpapers to each monitor
- **Named profiles** — save and switch between wallpaper configurations
- **Collections** — group profiles into collections and cycle through them with Prev/Next/Random buttons or an automatic slideshow timer
- **Cross-platform config** — share one `config.json` between Windows and Linux via monitor aliases
- **Schedule** — automatically switch profiles at specified times
- **System tray** — closing the window minimizes to the system tray; slideshows and the scheduler continue running in the background
- **Single instance** — launching the app again re-shows the existing window instead of starting a second process
- **Monitor aliases** — abstract away platform-specific device names (e.g., `\\?\DISPLAY#...` on Windows, `DP-2` on Linux) behind friendly names like "main", "left", "right"

## Supported Platforms

| Platform | Desktop Environment | Backend |
|----------|-------------------|---------|
| Windows  | Any               | COM `IDesktopWallpaper` API |
| Linux    | KDE Plasma 6      | `qdbus`/`qdbus6` + `kscreen-doctor` |

## Prerequisites

### Linux (KDE Plasma 6)

- WebKitGTK and tray build dependencies:
  - Debian/Ubuntu: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev`
  - Arch: `webkit2gtk-4.1 xdotool` (xdotool provides `libxdo`)
- `kscreen-doctor` — for monitor detection (part of `kscreen`/`libkscreen`); optional, a sysfs fallback exists
- `qdbus` or `qdbus6` — for reading/setting wallpapers via Plasma's scripting interface

### Windows

- No additional dependencies beyond a working Rust toolchain.

## Building

```bash
cargo build            # Debug build
cargo build --release  # Release build
```

## Running

```bash
cargo run
cargo run -- /path/to/config.json   # Use a custom config location
```

This launches the GUI where you can select a profile, view/change wallpapers per monitor, and apply them. An optional command-line argument specifies the config file path (defaults to `config.json` in the working directory).

Closing the window hides the app to the system tray rather than quitting. Use the tray icon menu to restore the window or quit.

## Configuration

Configuration is stored in `config.json`. The file has four sections: `platform_config`, `profiles`, `collections`, and `schedule`.

### Example

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
        "main": { "device": "DP-2", "width": 2560, "height": 1440 },
        "left": "DP-3"
      }
    }
  },
  "profiles": {
    "evening": {
      "monitor_wallpapers": {
        "main": "sunset/mountains.png",
        "left": "sunset/forest.png"
      },
      "tags": ["dark"]
    }
  },
  "collections": {
    "favorites": {
      "profiles": ["default", "evening"]
    }
  },
  "schedule": [
    { "profile_name": "evening", "hour": 18, "minute": 0, "enabled": true }
  ]
}
```

### Monitor map

Each alias in `monitor_map` maps to either:

- A **plain string** — the platform-specific device name (e.g., `"DP-3"`)
- A **detailed object** — `{ "device": "DP-2", "width": 2560, "height": 1440 }` for when you need to override the resolution displayed in the GUI (useful as a Plasma screen-index workaround)

### Wallpaper paths

Profiles store **relative** wallpaper paths. These are resolved against the current platform's `wallpaper_base_path` at apply time, which is what makes the config portable across machines.

## Landing page

A static project page built with React + [performative-ui](https://github.com/vorpus/performativeUI) lives in `web/`:

```bash
cd web
npm install
npm run dev      # local dev server
npm run build    # production build in web/dist
```

## License

This project does not currently specify a license.
