//! OS callbacks (tray, menu, single-instance window) run inside Win32 window
//! procedures, outside any GPUI context. They post an [`AppEvent`] here; the
//! shell drains the channel on the GPUI foreground executor. The channel
//! wakes the executor directly, so an idle app does no polling.

use std::sync::OnceLock;

use async_channel::{Receiver, Sender};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppEvent {
    /// Show the manager window, creating it if it is closed.
    OpenWindow,
    /// Flip the expansion engine on or off (tray "Enabled").
    ToggleEnabled,
    Quit,
}

static SENDER: OnceLock<Sender<AppEvent>> = OnceLock::new();

/// Create the channel. Call once, before any event source is installed.
pub fn init() -> Receiver<AppEvent> {
    let (tx, rx) = async_channel::unbounded();
    SENDER
        .set(tx)
        .expect("events::init must be called only once");
    rx
}

pub fn send(event: AppEvent) {
    if let Some(tx) = SENDER.get() {
        // Unbounded: this fails only after the shell has stopped receiving,
        // which happens only while the process exits.
        let _ = tx.try_send(event);
    }
}
