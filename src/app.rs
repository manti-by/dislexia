//! The `Dislexia` root entity: global state (settings, chat store, active
//! chat, streaming status), actions and wiring between UI and services.
//!
//! See `plan/01-architecture.md`. No HTTP and no file I/O here — blocking
//! work always goes through `cx.background_spawn` into the service modules.

use gpui_kit::component::{ActiveTheme, TitleBar, h_flex, v_flex};
use gpui_kit::*;

/// The root entity; every screen reads state from here.
#[derive(Default)]
pub struct Dislexia;

impl Dislexia {
    pub fn new() -> Self {
        Self
    }
}

impl Render for Dislexia {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .px_2()
                        .gap_2()
                        .child("Dislexia")
                        .text_color(cx.theme().muted_foreground),
                ),
            )
            // M1 replaces this placeholder with the sidebar + chat pane.
            .child(
                h_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .text_color(cx.theme().muted_foreground)
                    .child("Start a conversation with a local model ↓"),
            )
    }
}
