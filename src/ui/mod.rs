//! Window composition: opens the main window around the `Dislexia` root.
//!
//! Screens: `sidebar.rs` (previous chats), `chat_pane.rs` (transcript +
//! input), `message.rs` (one message row), `settings_ui.rs` (settings
//! dialog, M3), `title_bar.rs` (12px window controls). See
//! `plan/03-ui-layout.md`.

pub mod chat_pane;
pub mod message;
pub mod settings_ui;
pub mod sidebar;
pub mod title_bar;

use gpui_kit::component::TitleBar;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Pixels, Size, WindowBounds, WindowOptions, px,
};

use crate::app::Dislexia;

/// Default window size (`plan/03 §6`).
const WINDOW_SIZE: Size<Pixels> = Size {
    width: px(800.),
    height: px(600.),
};

/// Minimum window size: below the default 800×600, but still enough room
/// for the sidebar and a usable composer (`plan/03 §6`).
const WINDOW_MIN_SIZE: Size<Pixels> = Size {
    width: px(480.),
    height: px(360.),
};

/// Builds the default window options: kit title bar, 800×600 centered,
/// min size 480×360 (`plan/03 §6`). The native traffic lights are hidden;
/// `ui::title_bar` draws compact 12px controls instead, so AppKit must not
/// reserve or position its own.
pub fn window_options(cx: &App) -> WindowOptions {
    let mut options = TitleBar::window_options();
    options.window_bounds = Some(WindowBounds::centered(WINDOW_SIZE, cx));
    options.window_min_size = Some(WINDOW_MIN_SIZE);
    if let Some(titlebar) = &mut options.titlebar {
        titlebar.traffic_light_position = None;
    }
    options
}

/// Opens the main window hosting the `Dislexia` root entity, seeded with
/// the settings loaded by `main.rs`.
pub fn open_main_window(
    options: WindowOptions,
    settings: crate::settings::Settings,
    cx: &mut App,
) -> gpui_kit::Result<AnyWindowHandle> {
    let (handle, root) = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Dislexia::with_settings(settings, window, cx))
    })?;
    cx.set_global(crate::app::AppRoot {
        root: Some(root.downgrade()),
        window: Some(handle),
    });
    Ok(handle)
}
