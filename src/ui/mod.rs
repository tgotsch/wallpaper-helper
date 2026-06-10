use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::tray::{AppTray, TrayAction};
use crate::wallpaper_manager::WallpaperManager;
use background::RepeatingHandle;

pub mod app;
pub mod background;
mod alert_banner;
mod collections_view;
mod dialogs;
mod monitors_view;
mod preview;
mod profiles_view;
mod sidebar;
pub mod thumbs;

/// Values handed from main() into the root component via LaunchBuilder::with_context.
#[derive(Clone)]
pub struct AppInit {
    pub config_path: String,
    pub action_tx: UnboundedSender<TrayAction>,
    pub action_rx: Arc<Mutex<Option<UnboundedReceiver<TrayAction>>>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Profiles,
    Collections,
    Monitors,
}

/// Which name-input modal is open, if any.
#[derive(Clone, PartialEq)]
pub enum NameModalKind {
    NewProfile,
    SaveProfileAs,
    NewCollection,
}

/// Which confirmation modal is open, if any.
#[derive(Clone, PartialEq)]
pub enum ConfirmKind {
    DeleteProfile(String),
    DeleteCollection(String),
    DeleteAlias(String),
}

pub struct SlideshowState {
    /// Collection the slideshow runs (or is paused) on. Kept across pause so
    /// the tray can resume; cleared on full stop.
    pub collection: Option<String>,
    pub interval_min: u32,
    pub task: Option<RepeatingHandle>,
}

impl SlideshowState {
    pub fn is_running(&self) -> bool {
        self.task.is_some()
    }
}

impl Default for SlideshowState {
    fn default() -> Self {
        Self {
            collection: None,
            interval_min: 15,
            task: None,
        }
    }
}

/// Shared app state, provided once as context from the root component.
#[derive(Clone)]
pub struct AppCtx {
    pub manager: Signal<WallpaperManager>,
    pub config_path: Rc<str>,
    pub view: Signal<View>,
    pub selected_profile: Signal<Option<String>>,
    pub selected_collection: Signal<Option<String>>,
    /// Per-alias absolute wallpaper paths being edited for the selected
    /// profile. Seeded from the profile on selection; picker clicks change it.
    pub draft: Signal<HashMap<String, String>>,
    pub status: Signal<String>,
    pub slideshow: Signal<SlideshowState>,
    pub name_modal: Signal<Option<NameModalKind>>,
    pub confirm_modal: Signal<Option<ConfirmKind>>,
    pub window_visible: Signal<bool>,
    pub tray: Rc<AppTray>,
}

impl AppCtx {
    pub fn stop_slideshow(&self) {
        let mut slideshow = self.slideshow;
        let mut state = slideshow.write();
        if let Some(task) = state.task.take() {
            task.cancel();
        }
        state.collection = None;
        drop(state);
        self.tray.update_slideshow_state(false, "");
    }

    /// Stop the timer but keep the collection so the tray can resume.
    pub fn pause_slideshow(&self) {
        let mut slideshow = self.slideshow;
        let mut state = slideshow.write();
        if let Some(task) = state.task.take() {
            task.cancel();
        }
        let col = state.collection.clone().unwrap_or_default();
        drop(state);
        self.tray.update_slideshow_state(false, &col);
    }

    pub fn start_slideshow(&self, col_name: String, interval_min: u32) {
        let ctx = self.clone();
        let col = col_name.clone();
        let period = std::time::Duration::from_secs(interval_min as u64 * 60);
        let task = background::spawn_repeating(period, move || {
            let result = ctx.manager.clone().write().apply_next_in_collection(&col);
            match result {
                Some(name) => {
                    ctx.status.clone().set(format!("Applied: {}", name));
                    true
                }
                None => {
                    let mut slideshow = ctx.slideshow;
                    let mut state = slideshow.write();
                    state.task = None;
                    state.collection = None;
                    drop(state);
                    ctx.tray.update_slideshow_state(false, "");
                    false
                }
            }
        });

        let mut slideshow = self.slideshow;
        let mut state = slideshow.write();
        state.collection = Some(col_name.clone());
        state.interval_min = interval_min;
        state.task = Some(task);
        drop(state);
        self.tray.update_slideshow_state(true, &col_name);
    }

    /// Apply next/prev/random within a collection and update the status line.
    pub fn apply_in_collection(&self, col_name: &str, direction: CollectionStep) {
        let mut manager = self.manager;
        let result = {
            let mut mgr = manager.write();
            match direction {
                CollectionStep::Next => mgr.apply_next_in_collection(col_name),
                CollectionStep::Prev => mgr.apply_prev_in_collection(col_name),
                CollectionStep::Random => mgr.apply_random_from_collection(col_name),
            }
        };
        if let Some(name) = result {
            self.status.clone().set(format!("Applied: {}", name));
        }
    }

    /// Select a profile in the Profiles view and seed the editor draft from it.
    pub fn select_profile(&self, name: Option<String>) {
        let draft = match &name {
            Some(profile_name) => draft_from_profile(&self.manager.clone().read(), profile_name),
            None => HashMap::new(),
        };
        self.selected_profile.clone().set(name);
        self.draft.clone().set(draft);
    }

    /// Whether the draft differs from the selected profile's saved wallpapers.
    pub fn draft_dirty(&self) -> bool {
        let selected = self.selected_profile.clone();
        let selected = selected.read();
        let Some(profile_name) = selected.as_ref() else {
            return false;
        };
        let manager = self.manager.clone();
        let saved = draft_from_profile(&manager.read(), profile_name);
        *self.draft.clone().read() != saved
    }

    /// Write the draft's wallpapers into `profile_name` (creating it if
    /// needed) and persist the config.
    pub fn save_draft_to(&self, profile_name: &str) {
        let draft = self.draft.clone().read().clone();
        let mut manager = self.manager;
        let mut mgr = manager.write();
        if !mgr.profiles.contains_key(profile_name) {
            mgr.create_profile(profile_name);
        }
        let aliases = mgr.aliases.clone();
        for alias in &aliases {
            if let Some(path) = draft.get(alias) {
                if !path.is_empty() {
                    mgr.set_wallpaper_in_profile(profile_name, alias, path);
                }
            }
        }
        mgr.save_config(&self.config_path);
    }
}

#[derive(Clone, Copy)]
pub enum CollectionStep {
    Next,
    Prev,
    Random,
}

/// The selected profile's wallpapers as absolute paths per alias (empty string
/// when the profile has no entry for an alias).
pub fn draft_from_profile(manager: &WallpaperManager, profile_name: &str) -> HashMap<String, String> {
    let profile = manager.profiles.get(profile_name);
    manager
        .aliases
        .iter()
        .map(|alias| {
            let abs = profile
                .and_then(|p| p.monitor_wallpapers.get(alias))
                .map(|rel| manager.resolve_wallpaper_path(rel))
                .unwrap_or_default();
            (alias.clone(), abs)
        })
        .collect()
}

/// The profile a collection's cycle position currently points at.
pub fn current_collection_profile(manager: &WallpaperManager, col_name: &str) -> Option<String> {
    let valid = manager.get_valid_collection_profiles(col_name);
    if valid.is_empty() {
        return None;
    }
    let idx = manager
        .collection_cycle_indices
        .get(col_name)
        .copied()
        .unwrap_or(0);
    Some(valid[idx.min(valid.len() - 1)].clone())
}

/// URL for the wallpaper asset handler: serves a cached JPEG of the local
/// file downscaled to at most `width` px (see `thumbs`). Omitting `?w=` on a
/// handler request serves the raw file, but the UI always wants it bounded.
pub fn wallpaper_thumb_url(abs_path: &str, width: u32) -> String {
    format!(
        "/wallpaper/{}?w={}",
        utf8_percent_encode(abs_path, NON_ALPHANUMERIC),
        width
    )
}

/// Port of the GTK update_warning_alert_for_profile: alias mismatches and
/// missing wallpaper files for a profile.
pub fn warning_for_profile(manager: &WallpaperManager, profile_name: &str) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(info) = manager.check_profile_mismatch(profile_name) {
        if !info.extra_aliases.is_empty() {
            parts.push(format!(
                "Profile has unknown aliases: {}",
                info.extra_aliases.join(", ")
            ));
        }
        if !info.missing_aliases.is_empty() {
            parts.push(format!(
                "No wallpaper set for: {}",
                info.missing_aliases.join(", ")
            ));
        }
    }

    if let Some(profile) = manager.profiles.get(profile_name) {
        let mut missing_files = Vec::new();
        for (alias, relative_path) in &profile.monitor_wallpapers {
            if relative_path.is_empty() {
                continue;
            }
            let abs = manager.resolve_wallpaper_path(relative_path);
            if !std::path::Path::new(&abs).exists() {
                missing_files.push(format!("{} ({})", alias, relative_path));
            }
        }
        missing_files.sort();
        if !missing_files.is_empty() {
            parts.push(format!("Image not found for: {}", missing_files.join(", ")));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// Sorted profile names.
pub fn sorted_profiles(manager: &WallpaperManager) -> Vec<String> {
    let mut names: Vec<String> = manager.profiles.keys().cloned().collect();
    names.sort();
    names
}

/// Sorted collection names.
pub fn sorted_collections(manager: &WallpaperManager) -> Vec<String> {
    let mut names: Vec<String> = manager.collections.keys().cloned().collect();
    names.sort();
    names
}
