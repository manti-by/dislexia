//! Dislexia — a macOS menu-bar chat client for local Ollama models.
//!
//! Boot order only: logging → theme → settings/history → tray → open window.
//! All logic lives in `src/lib.rs` behind `dislexia::`.

use gpui_kit::component::TitleBar;
use gpui_kit::*;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    application().run(|cx| {
        gpui_kit::init(cx);

        let root = cx.new(|_| dislexia::app::Dislexia::new());

        let options = WindowOptions {
            window_min_size: Some(Size {
                width: px(720.),
                height: px(480.),
            }),
            ..TitleBar::window_options()
        };
        dislexia::ui::open_main_window(root, options, cx).expect("failed to open main window");

        cx.activate(true);
    });
}
