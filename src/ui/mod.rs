//! Window composition: opens the main window around the `Dislexia` root.
//!
//! Screens: `sidebar.rs` (previous chats), `chat_pane.rs` (transcript +
//! input), `message.rs` (one message row), `settings_ui.rs` (settings
//! dialog). See `plan/03-ui-layout.md`.

mod chat_pane;
mod message;
mod settings_ui;
mod sidebar;

use gpui_kit::{App, Entity, WindowOptions};

use crate::app::Dislexia;

/// Opens (or re-opens) the main window hosting the given root entity.
pub fn open_main_window(
    root: Entity<Dislexia>,
    options: WindowOptions,
    cx: &mut App,
) -> gpui_kit::Result<(gpui_kit::AnyWindowHandle, Entity<Dislexia>)> {
    gpui_kit::open_window(options, cx, |_, _| root)
}
