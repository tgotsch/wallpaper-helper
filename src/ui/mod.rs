use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::tray::{AppTray, TrayAction};
use crate::wallpaper_manager::WallpaperManager;
use background::RepeatingHandle;

pub mod app;
pub mod background;
mod alert_banner;
mod collection_controls;
mod collections_modal;
mod dialogs;
mod monitor_grid;
mod profile_selector;

#[derive(Clone, Debug, PartialEq)]
pub enum DropdownEntry {
    Profile(String),
    Collection(String),
}

impl DropdownEntry {
    pub fn display_label(&self) -> String {
        match self {
            DropdownEntry::Profile(name) => name.clone(),
            DropdownEntry::Collection(name) => format!("[Collection] {}", name),
        }
    }
}

/// Values handed from main() into the root component via LaunchBuilder::with_context.
#[derive(Clone)]
pub struct AppInit {
    pub config_path: String,
    pub action_tx: UnboundedSender<TrayAction>,
    pub action_rx: Arc<Mutex<Option<UnboundedReceiver<TrayAction>>>>,
}

pub struct SlideshowState {
    /// Collection the slideshow runs (or is paused) on. Kept across pause so
    /// the tray can resume; cleared on full stop.
    pub collection: Option<String>,
    pub interval_min: u32,
    pub task: Option<RepeatingHandle>,
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

impl SlideshowState {
    pub fn is_running(&self) -> bool {
        self.task.is_some()
    }
}

/// Shared app state, provided once as context from the root component.
#[derive(Clone)]
pub struct AppCtx {
    pub manager: Signal<WallpaperManager>,
    pub config_path: Rc<str>,
    pub selected: Signal<Option<DropdownEntry>>,
    /// Per-alias wallpaper paths used when creating a new profile: seeded from
    /// the wallpapers active at startup, overwritten by file-picker choices.
    pub pending: Signal<HashMap<String, String>>,
    /// Picker choices shown in the grid until the selection changes.
    pub display_overrides: Signal<HashMap<String, String>>,
    pub status: Signal<String>,
    pub slideshow: Signal<SlideshowState>,
    pub show_collections_modal: Signal<bool>,
    pub show_new_profile_dialog: Signal<bool>,
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

    /// Switch the dropdown selection: stops any slideshow, drops picker
    /// overrides, and resets the status line (collections show their current
    /// cycle entry). Mirrors the GTK selected-notify + update_ui path.
    pub fn select_entry(&self, entry: Option<DropdownEntry>) {
        self.stop_slideshow();
        self.display_overrides.clone().write().clear();
        let status = match &entry {
            Some(DropdownEntry::Collection(_)) => {
                let manager = self.manager;
                let mgr = manager.read();
                match displayed_profile_name(&mgr, &entry) {
                    Some(profile) => format!("Current: {}", profile),
                    None => String::new(),
                }
            }
            _ => String::new(),
        };
        self.selected.clone().set(entry);
        self.status.clone().set(status);
    }
}

#[derive(Clone, Copy)]
pub enum CollectionStep {
    Next,
    Prev,
    Random,
}

/// The profile whose wallpapers the grid should display for the current
/// selection: the profile itself, or the collection's current cycle entry.
pub fn displayed_profile_name(
    manager: &WallpaperManager,
    selected: &Option<DropdownEntry>,
) -> Option<String> {
    match selected {
        Some(DropdownEntry::Profile(name)) => Some(name.clone()),
        Some(DropdownEntry::Collection(col_name)) => {
            let valid = manager.get_valid_collection_profiles(col_name);
            if valid.is_empty() {
                None
            } else {
                let idx = manager
                    .collection_cycle_indices
                    .get(col_name)
                    .copied()
                    .unwrap_or(0);
                Some(valid[idx.min(valid.len() - 1)].clone())
            }
        }
        None => None,
    }
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
