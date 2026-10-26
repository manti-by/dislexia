//! The 12px window controls drawn in the app title bar
//! (`wiki/pages/2026-10-26-03-ui-layout.md` §6). macOS 27's system traffic lights are 14px
//! and AppKit does not let a window resize them, so the native buttons are
//! hidden (`platform::macos::hide_window_buttons`) and these take their
//! place: same colours, same order, hover glyphs, native click behaviour.

use gpui_kit::component::{ActiveTheme, Icon, IconName, h_flex};
use gpui_kit::*;

use crate::app::Dislexia;

/// Diameter of one control; matches the older macOS traffic lights.
const CONTROL_SIZE: f32 = 12.;
/// Gap between two neighbouring controls (native spacing is 20px centre to
/// centre, i.e. 8px between 12px circles).
const CONTROL_GAP: f32 = 8.;
/// Hover glyph size inside a control.
const GLYPH_SIZE: f32 = 8.;

#[derive(Clone, Copy)]
enum Control {
    Close,
    Minimize,
    Zoom,
}

/// The three window controls, left to right, at the leading edge of the
/// title bar. Inactive windows dim them to one neutral colour, like the
/// native controls do.
pub fn render(active: bool, cx: &mut Context<Dislexia>) -> impl IntoElement + use<> {
    // The traffic lights are the only chromatic colors in the app; inactive
    // windows dim them to the neutral muted grey, like native macOS does.
    let (close, minimize, zoom) = if active {
        crate::theme::window_control_colors()
    } else {
        let muted = cx.theme().muted_foreground;
        (muted, muted, muted)
    };
    h_flex()
        .id("window-controls")
        .group("window-controls")
        .flex_shrink_0()
        .items_center()
        .gap(px(CONTROL_GAP))
        .child(control(Control::Close, IconName::WindowClose, close, cx))
        .child(control(
            Control::Minimize,
            IconName::WindowMinimize,
            minimize,
            cx,
        ))
        .child(control(Control::Zoom, IconName::WindowMaximize, zoom, cx))
}

fn control(
    control: Control,
    icon: IconName,
    color: Hsla,
    cx: &mut Context<Dislexia>,
) -> impl IntoElement + use<> {
    // The glyph must contrast with the coloured circle in both themes.
    let glyph_color = cx.theme().background;
    let id = match control {
        Control::Close => "window-close",
        Control::Minimize => "window-minimize",
        Control::Zoom => "window-zoom",
    };

    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(CONTROL_SIZE))
        .rounded_full()
        .bg(color)
        // Keep a click from starting a window drag through the title bar.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |_this, _, window, _cx| match control {
            Control::Close => window.remove_window(),
            Control::Minimize => window.minimize_window(),
            Control::Zoom => window.zoom_window(),
        }))
        .child(
            div()
                .invisible()
                .group_hover("window-controls", |style| style.visible())
                .text_color(glyph_color)
                .child(Icon::new(icon).size(px(GLYPH_SIZE))),
        )
}
