//! Repeating background work (tray polling, scheduler, slideshow).
//!
//! These must NOT run as VirtualDom tasks on Linux: while the window is
//! hidden, WebKitGTK suspends the page, pending render edits can never be
//! flushed, and dioxus's `poll_vdom` stops polling *all* spawned futures until
//! the window is shown again (the render pipeline self-heals on show). On
//! Linux tao's event loop is a glib main loop, so glib timers keep firing
//! while the window is hidden; each tick is wrapped in the dioxus runtime
//! scope so signal writes behave exactly like writes from event handlers.
//!
//! On Windows there is no glib loop; ticks run as VirtualDom tasks (the
//! webview there is driven by WebView2, and tray events additionally arrive
//! through dioxus's own event-loop hooks).

use std::time::Duration;

/// Handle to a repeating background tick. Dropping it does NOT cancel the
/// tick; call `cancel()`. If the tick callback returns `false` it stops
/// itself, in which case the handle must not be cancelled afterwards (the
/// owner is expected to drop/clear it from within the callback).
pub struct RepeatingHandle(HandleInner);

#[cfg(target_os = "linux")]
type HandleInner = glib::SourceId;
#[cfg(windows)]
type HandleInner = dioxus::dioxus_core::Task;

impl RepeatingHandle {
    pub fn cancel(self) {
        #[cfg(target_os = "linux")]
        self.0.remove();
        #[cfg(windows)]
        self.0.cancel();
    }
}

/// Run `tick` every `period` until it returns `false` or the handle is
/// cancelled. Must be called from within the dioxus runtime (component init
/// or another background tick).
#[cfg(target_os = "linux")]
pub fn spawn_repeating(
    period: Duration,
    mut tick: impl FnMut() -> bool + 'static,
) -> RepeatingHandle {
    let runtime = dioxus::dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let id = glib::timeout_add_local(period, move || {
        if runtime.in_scope(scope, &mut tick) {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });
    RepeatingHandle(id)
}

#[cfg(windows)]
pub fn spawn_repeating(
    period: Duration,
    mut tick: impl FnMut() -> bool + 'static,
) -> RepeatingHandle {
    let task = dioxus::prelude::spawn(async move {
        loop {
            tokio::time::sleep(period).await;
            if !tick() {
                break;
            }
        }
    });
    RepeatingHandle(task)
}
