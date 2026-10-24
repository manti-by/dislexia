//! Window composition: opens the main window around the `Dislexia` root.
//!
//! Screens: `sidebar.rs` (previous chats), `chat_pane.rs` (transcript +
//! input), `message.rs` (one message row), `settings_ui.rs` (settings
//! dialog, M3). See `plan/03-ui-layout.md`.

pub mod chat_pane;
pub mod message;
pub mod settings_ui;
pub mod sidebar;

use gpui_kit::component::TitleBar;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Pixels, Size, WindowBounds, WindowOptions, px,
};

use crate::app::Dislexia;

/// Default window size (`plan/03 §6`).
const WINDOW_SIZE: Size<Pixels> = Size {
    width: px(1000.),
    height: px(680.),
};

/// Builds the default window options: kit title bar, 1000×680 centered,
/// min size 720×480 (`plan/03 §6`).
pub fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(WINDOW_SIZE, cx)),
        window_min_size: Some(Size {
            width: px(720.),
            height: px(480.),
        }),
        ..TitleBar::window_options()
    }
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
