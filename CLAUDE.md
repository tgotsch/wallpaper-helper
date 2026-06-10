# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build          # Debug build
cargo build --release # Release build
cargo run            # Run the app (launches Dioxus desktop GUI in a webview)
cargo test --verbose # Run tests (no tests exist yet)
```

CI runs `cargo build --verbose && cargo test --verbose` on both Ubuntu and Windows, plus an `npm ci && npm run build` job for `web/` (see `.github/workflows/rust.yml`). Plain `cargo` is sufficient — the `dx` CLI is not used (no `asset!()` macros; CSS is embedded via `include_str!`).

### Linux requirements

On Linux (KDE Plasma 6), ensure these are available:
- Build/link deps: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev` (Debian/Ubuntu names; on Arch, `webkit2gtk-4.1` and `xdotool` for libxdo)
- `kscreen-doctor` — for monitor detection (part of `kscreen` / `libkscreen`); optional, sysfs fallback exists
- `qdbus` or `qdbus6` — for reading/setting wallpapers via Plasma's scripting interface

### Landing page (`web/`)

Static project page built with Vite + React + the `performative-ui` component library. `cd web && npm install && npm run build` (or `npm run dev`). Not wired to the app; purely a website.

## Architecture

This is a **cross-platform desktop wallpaper manager** with a **Dioxus 0.7 desktop GUI** (webview renderer — wry/tao). It lets users create named wallpaper profiles that map per-monitor wallpaper images, then apply them. A single `config.json` can be shared across platforms (e.g., on a network drive) using user-defined monitor aliases and per-platform settings.

### Platform backends

Platform-specific code lives behind a `WallpaperBackend` trait in `src/backend/`:

- **`src/backend/mod.rs`** — Defines the platform-agnostic `MonitorInfo` struct (`device_name`, `width`, `height`, `is_primary`), the `WallpaperBackend` trait (`refresh_monitors`, `get_current_wallpaper`, `set_wallpaper`), and a `create_backend()` factory using `#[cfg]` gates.
- **`src/backend/windows.rs`** (`#[cfg(windows)]`) — Windows backend using COM `IDesktopWallpaper` interface. Monitors detected via `EnumDisplayMonitors` + `IDesktopWallpaper` device paths.
- **`src/backend/kde.rs`** (`#[cfg(target_os = "linux")]`) — KDE Plasma 6 backend. Monitor detection via `kscreen-doctor --outputs` with sysfs fallback (`/sys/class/drm/card*-*`) when kscreen-doctor fails (common on Plasma 6). Wallpaper get/set via `qdbus`/`qdbus6` `evaluateScript` calls to `org.kde.plasmashell`, using Plasma 6's `currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General']` before `readConfig`/`writeConfig`. Maps connector names to Plasma screen indices via sequential assignment (alphabetical order). Note: Plasma's screen index ordering may not match connector name ordering — the config's `monitor_map` can compensate by mapping aliases to whichever connector produces the correct screen index.

### Core modules

- **`src/main.rs`** — Entry point. Initializes logging, enforces **single instance** via an `interprocess` local socket (`com.wallpaperhelper.app.sock`, abstract namespace on Linux / named pipe on Windows): a second launch writes "SHOW" to the socket and exits, and the primary's listener thread forwards `TrayAction::ShowWindow` into the app's action channel. Configures and launches the Dioxus desktop app: 1000x1000 window, no menubar, `WindowCloseBehaviour::WindowHides` (close-to-tray). Config path comes from argv[1] (default `config.json`), passed to the root component through `LaunchBuilder::with_context` as `ui::AppInit` (also carries the tray action channel).

- **`src/ui/`** — Dioxus components. Sidebar-navigation app shell with three views (`View::Profiles | Collections | Monitors`):
  - **`app.rs`** — Root `App` component. Owns all state as signals: `Signal<WallpaperManager>` (works despite `!Clone`/`!Send` because Dioxus desktop is single-threaded with `UnsyncStorage`), `view`, `selected_profile`, `selected_collection`, `draft`, slideshow, modal state, window visibility. Provides `AppCtx` via context; a `use_effect` keeps selections valid when profiles/collections are deleted. Registers: a `use_asset_handler("wallpaper", ...)` that serves local image files to the webview (`img src="/wallpaper/<percent-encoded-abs-path>"`, see `wallpaper_url()`); a `use_wry_event_handler` for `CloseRequested` (tracks hidden state + sends "still running" notification via `notify-rust`); the tray/single-instance **action drain** (100ms tick over a tokio unbounded channel → `handle_tray_action`) and the **scheduler** (30s tick calling `check_and_apply_schedule()`), both via `background::spawn_repeating` so they keep running while the window is hidden. On Windows only, wires `use_tray_menu_event_handler`/`use_tray_icon_event_handler` into the action channel.
  - **`background.rs`** — `spawn_repeating(period, tick)` → `RepeatingHandle`. CRITICAL: background work must NOT run as VirtualDom futures on Linux. While the window is hidden, WebKitGTK suspends the page, pending render edits are never acked, and dioxus's `poll_vdom` returns early — freezing *all* spawned futures until the window is shown again (the pipeline self-heals on show). On Linux ticks run as `glib::timeout_add_local` on tao's gtk main loop (wrapped in `Runtime::in_scope` so signal writes work like event handlers); on Windows they run as VirtualDom tasks (untested caveat: WebView2 hidden-window behavior).
  - **`mod.rs`** — Shared types (`AppInit`, `AppCtx`, `View`, `SlideshowState`, `NameModalKind`, `ConfirmKind`) and logic: `select_profile` (seeds the editor `draft` from the profile), `draft_dirty`/`save_draft_to` (profile editing), `start_slideshow`/`pause_slideshow`/`stop_slideshow` (pause keeps the collection for tray resume, stop clears it), `apply_in_collection`, `draft_from_profile`, `current_collection_profile`, `warning_for_profile`, `wallpaper_url`.
  - **`sidebar.rs`** — Nav (Profiles/Collections/Monitors) + slideshow status chip (running/paused + collection name).
  - **`profiles_view.rs`** — Profile cards (name, warning dot, mini `PreviewStrip`) + editor pane: per-monitor `EditorGrid` with click-to-pick (`rfd::AsyncFileDialog` → `draft`), Apply (disabled while dirty), Save changes / Save as new / Discard / Delete. "New profile" seeds the new profile from the current desktop wallpapers, then opens it in the editor.
  - **`collections_view.rs`** — Collection cards + detail pane: Prev/Random/Next, slideshow interval + Start/Stop (running on a different collection is taken over, not stopped, when switching selection), member rows with `PreviewStrip` and missing/current badges, add-profile flow that previews the candidate profile before adding.
  - **`monitors_view.rs`** — Edits the **current platform's** config: `wallpaper_base_path`, Rescan (`refresh_monitors()`), detected-monitor strip (primary/mapped badges), alias rows (device input with `datalist` of detected connectors, optional resolution override → `MonitorMapping::Detailed`, delete with confirm), add-alias row. Mutations go through `current_platform_config_mut()` + `sync_aliases()` + `save_config()`.
  - **`preview.rs`** — `EditorGrid` (large editable tiles) and `PreviewStrip` (mini per-alias thumbnails) — both render via the asset handler and skip `img`s while the window is hidden.
  - **`dialogs.rs`** — `NameModal` (new profile / save-as / new collection via `NameModalKind`) and `ConfirmModal` (deletes via `ConfirmKind`).
  - **`alert_banner.rs`** — Warning banner (takes the text as a prop).
  - Styling: `assets/main.css`, embedded with `include_str!` into a `style` element (no asset pipeline). Design tokens as CSS custom properties; dark palette by default with a light override via `@media (prefers-color-scheme: light)`.
  - NOTE: screenshots of the webview content come back blank under Wayland (WebKitGTK renders into a subsurface that window capture misses) — verify UI state via `document::eval` DOM checks instead.

- **`src/tray.rs`** — System tray with a common `TrayAction` enum (`ShowWindow`, `Quit`, `NextWallpaper`, `PrevWallpaper`, `ToggleSlideshow`); both platforms push actions into a tokio unbounded channel consumed by the root component:
  - **Windows**: builds the tray menu with the **`dioxus::desktop::trayicon` re-export** of `tray-icon`. IMPORTANT: dioxus-desktop installs global `tray_icon` event handlers at startup, so `MenuEvent::receiver()` never fires — menu events must be consumed via `dioxus::desktop::use_tray_menu_event_handler` and mapped through `AppTray::map_menu_event`. Never add a direct `tray-icon` dependency (a second crate copy would not be routed).
  - **Linux**: `ksni` crate (StatusNotifierItem via D-Bus, blocking mode on its own thread — unaffected by dioxus's handlers). Menu rebuilt from shared slideshow state.
  - Both expose `AppTray::new(sender)` and `update_slideshow_state(active, collection)`.

- **`src/wallpaper_manager.rs`** — Platform-agnostic core logic. `WallpaperManager` holds:
  - `monitors: Vec<MonitorInfo>` — detected displays (delegated to backend)
  - `aliases: Vec<String>` — user-defined monitor names (e.g., "main", "left", "right"), derived from the current platform's `monitor_map` keys
  - `platform_configs: PlatformConfigs` — per-platform settings (base paths, monitor mappings)
  - `profiles: HashMap<String, WallpaperProfile>` — named profiles mapping aliases to relative wallpaper paths
  - `collections: HashMap<String, ProfileCollection>` + `collection_cycle_indices` — profile groups with Prev/Next/Random cycling
  - `schedule: Vec<ScheduleEntry>` — time-based profile switching
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
  - `check_and_apply_schedule()` — compares current time against enabled schedule entries; applies the matching profile and returns its name, with a minute-level guard to prevent re-triggering
  - Collection methods: `create_collection`, `delete_collection`, `add_profile_to_collection`, `remove_profile_from_collection`, `get_valid_collection_profiles`, `apply_next_in_collection`, `apply_prev_in_collection`, `apply_random_from_collection`

- **`src/logging.rs`** — `DualLogger` writing to stdout and `wallpaper_helper.log`.

- **`src/wallpaper_source.rs`** — WIP/incomplete trait-based abstraction for wallpaper sources. Currently commented out.

### Key patterns

- Profiles use **aliases** (e.g., "main", "left", "right") as monitor keys and **relative paths** for wallpapers. Platform-specific device names and base paths are in `platform_config`.
- All mutable state lives in Dioxus `Signal`s created in the root component and shared via `AppCtx` context; derived values are `use_memo`s. `WallpaperManager` lives directly in a `Signal` (single-threaded desktop renderer).
- **System tray daemon**: Closing the window hides it (`WindowCloseBehaviour::WindowHides`); slideshow/scheduler ticks keep running because they live on the glib main loop (Linux), not the VirtualDom. Tray "Quit" must first `set_close_behavior(WindowCloses)` before `window.close()` — `CloseWindow` events are routed through the close behaviour, so `WindowHides` would otherwise swallow the quit.
- **Single instance**: `interprocess` local socket; a second launch signals the first to show its window (replaces the old GTK Application ID mechanism).
- Background work goes through `ui::background::spawn_repeating` (glib timers on Linux, VirtualDom tasks on Windows) — never plain `use_future`/`dioxus::spawn` loops, which freeze while the window is hidden on Linux (see `src/ui/background.rs`).
- Logging: our `DualLogger` handles `log::` macros (stdout + `wallpaper_helper.log`). Dioxus separately auto-initializes a `tracing` subscriber — DEBUG level in debug builds (chatty zbus/ksni output on stdout), INFO in release; `RUST_LOG` overrides it.

### Config file format (`config.json`)

JSON file with `platform_config`, `profiles`, `collections`, and `schedule` top-level keys, serialized via `serde`. Profiles use aliases and relative wallpaper paths. Each platform section maps aliases to platform-specific device names and provides the wallpaper base path.

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
  "collections": {
    "favorites": {
      "profiles": ["profile_name"]
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
