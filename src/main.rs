//! Dislexia — a macOS menu-bar chat client for local Ollama models.
//!
//! Boot order only: logging → theme → settings → tray → open window.
//! All logic lives in `src/lib.rs` behind `dislexia::`.

use dislexia::platform::macos::DockPolicy;
use gpui_kit::*;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    application().run(|cx| {
        gpui_kit::init(cx);
        dislexia::theme::init(cx);

        // Global shortcuts (`plan 03 §7`); per-window actions dispatch to the
        // root entity's `on_action` handlers.
        cx.bind_keys([
            KeyBinding::new("cmd-n", dislexia::app::NewChat, None),
            KeyBinding::new("cmd-,", dislexia::app::OpenSettings, None),
            KeyBinding::new("cmd-w", dislexia::app::HideWindow, None),
            KeyBinding::new("cmd-q", dislexia::app::QuitApp, None),
            KeyBinding::new("escape", dislexia::app::EscapePressed, None),
        ]);

        // Tray must outlive every window — it leaks itself on purpose
        // (dropping a tray icon removes it).
        dislexia::tray::init(cx);

        // Closing the last window hides the Dock icon but keeps the app and
        // tray alive (`plan 08 §4`); quitting stays explicit.
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                dislexia::platform::macos::set_dock_policy(DockPolicy::Accessory);
            }
        })
        .detach();

        // Settings load before the window opens (`plan 02 §4` boot order).
        let settings = dislexia::settings::load();
        dislexia::ui::open_main_window(
            dislexia::ui::window_options(cx, settings.window_frame),
            settings,
            cx,
        )
        .expect("failed to open main window");

        cx.activate(true);
    });
}
