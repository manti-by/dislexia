//! Dislexia — a macOS menu-bar chat client for local Ollama models.
//!
//! Boot order only: logging → theme → settings/history → tray → open window.
//! All logic lives in `src/lib.rs` behind `dislexia::`.

use gpui_kit::*;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    application().run(|cx| {
        gpui_kit::init(cx);
        dislexia::theme::init(cx);

        // Global shortcuts (`plan/03 §7`); per-window actions dispatch to the
        // root entity's `on_action` handlers.
        cx.bind_keys([
            KeyBinding::new("cmd-n", dislexia::app::NewChat, None),
            KeyBinding::new("cmd-,", dislexia::app::OpenSettings, None),
        ]);

        // Settings load before the window opens (`plan/02 §4` boot order).
        let settings = dislexia::settings::load();
        dislexia::ui::open_main_window(dislexia::ui::window_options(cx), settings, cx)
            .expect("failed to open main window");

        cx.activate(true);
    });
}
