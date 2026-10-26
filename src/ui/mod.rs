//! Window composition: opens the main window around the `Dislexia` root.
//!
//! Screens: `sidebar.rs` (previous chats), `chat_pane.rs` (transcript +
//! input), `message.rs` (one message row), `settings_ui.rs` (settings
//! dialog, M3), `title_bar.rs` (12px window controls). See
//! `wiki/pages/2026-10-26-03-ui-layout.md`.

pub mod chat_pane;
pub mod message;
pub mod settings_ui;
pub mod sidebar;
pub mod title_bar;

use gpui_kit::component::TitleBar;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Pixels, Size, WindowBounds, WindowOptions,
    point, px, size,
};

use crate::app::Dislexia;
use crate::settings::WindowFrame;

/// Default window size (`plan 03 §6`).
const WINDOW_SIZE: Size<Pixels> = Size {
    width: px(800.),
    height: px(600.),
};

/// Minimum window size: below the default 800×600, but still enough room
/// for the sidebar and a usable composer (`plan 03 §6`).
const WINDOW_MIN_SIZE: Size<Pixels> = Size {
    width: px(480.),
    height: px(360.),
};

/// How much of a restored window must stay inside its display, so the title
/// bar is always grabbable even when the saved frame hung off an edge.
const MIN_VISIBLE: f32 = 80.;

/// Builds the window options: kit title bar, the remembered frame or 800×600
/// centered, min size 480×360 (`plan 03 §6`). The native traffic lights are
/// hidden; `ui::title_bar` draws compact 12px controls instead, so AppKit must
/// not reserve or position its own.
pub fn window_options(cx: &App, frame: Option<WindowFrame>) -> WindowOptions {
    let mut options = TitleBar::window_options();
    options.window_min_size = Some(WINDOW_MIN_SIZE);
    if let Some(titlebar) = &mut options.titlebar {
        titlebar.traffic_light_position = None;
    }

    // A saved frame is only reusable on the display it was saved on: the
    // coordinates are relative to that screen (`plan 03 §6`).
    let restored = frame.and_then(|frame| {
        let display = frame.display?;
        let display = cx
            .displays()
            .into_iter()
            .find(|candidate| candidate.uuid().is_ok_and(|uuid| uuid == display))?;
        Some((frame, display.id(), display.visible_bounds()))
    });

    match restored {
        Some((frame, display_id, display_bounds)) => {
            options.display_id = Some(display_id);
            options.window_bounds = Some(WindowBounds::Windowed(fit_frame(frame, display_bounds)));
        }
        None => options.window_bounds = Some(WindowBounds::centered(WINDOW_SIZE, cx)),
    }
    options
}

/// Clamps a saved frame onto its display: never smaller than the minimum
/// window size, never larger than the screen, and always leaving `MIN_VISIBLE`
/// points of the window inside the display on both axes.
fn fit_frame(frame: WindowFrame, display: Bounds<Pixels>) -> Bounds<Pixels> {
    let left = display.origin.x.as_f32();
    let top = display.origin.y.as_f32();
    let right = left + display.size.width.as_f32();
    let bottom = top + display.size.height.as_f32();

    // A corrupt or first-run frame falls back to the default size; the
    // `min`/`max` order is deliberate so a degenerate display cannot panic.
    let width = frame
        .width
        .max(WINDOW_MIN_SIZE.width.as_f32())
        .min(display.size.width.as_f32());
    let height = frame
        .height
        .max(WINDOW_MIN_SIZE.height.as_f32())
        .min(display.size.height.as_f32());

    let x = frame
        .x
        .min(right - MIN_VISIBLE)
        .max(left - (width - MIN_VISIBLE));
    let y = frame.y.min(bottom - MIN_VISIBLE).max(top);

    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1920×1080 primary display at the origin.
    fn display() -> Bounds<Pixels> {
        Bounds::new(point(px(0.), px(0.)), size(px(1920.), px(1080.)))
    }

    fn frame(x: f32, y: f32, width: f32, height: f32) -> WindowFrame {
        WindowFrame {
            x,
            y,
            width,
            height,
            display: None,
        }
    }

    #[test]
    fn saved_frame_is_restored_verbatim() {
        let bounds = fit_frame(frame(120., 90., 900., 700.), display());
        assert_eq!(bounds.origin, point(px(120.), px(90.)));
        assert_eq!(bounds.size, size(px(900.), px(700.)));
    }

    #[test]
    fn tiny_frame_is_grown_to_the_minimum_size() {
        let bounds = fit_frame(frame(10., 10., 100., 100.), display());
        assert_eq!(bounds.size, WINDOW_MIN_SIZE);
    }

    #[test]
    fn oversized_frame_is_shrunk_to_the_display() {
        let bounds = fit_frame(frame(0., 0., 4000., 3000.), display());
        assert_eq!(bounds.size, size(px(1920.), px(1080.)));
    }

    #[test]
    fn frame_off_the_right_edge_keeps_a_grabbable_column_visible() {
        let bounds = fit_frame(frame(5000., 100., 800., 600.), display());
        assert_eq!(bounds.origin.x.as_f32(), 1920. - MIN_VISIBLE);
        assert_eq!(bounds.origin.y.as_f32(), 100.);
    }

    #[test]
    fn frame_off_the_bottom_edge_keeps_its_title_bar_visible() {
        let bounds = fit_frame(frame(100., 5000., 800., 600.), display());
        assert_eq!(bounds.origin.y.as_f32(), 1080. - MIN_VISIBLE);
    }

    #[test]
    fn frame_above_the_top_edge_is_pushed_down() {
        let bounds = fit_frame(frame(100., -80., 800., 600.), display());
        assert_eq!(bounds.origin.y.as_f32(), 0.);
    }

    #[test]
    fn negative_origin_from_a_second_display_is_kept() {
        // A display to the left of the primary one has negative coordinates.
        let left_display = Bounds::new(point(px(-1440.), px(0.)), size(px(1440.), px(900.)));
        let bounds = fit_frame(frame(-1200., 200., 800., 600.), left_display);
        assert_eq!(bounds.origin, point(px(-1200.), px(200.)));
    }
}
