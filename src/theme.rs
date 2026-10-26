//! OS appearance tracking + the only place colors are assigned
//! (`plan/07-theme.md`). The palette is greyscale; the only chromatic values
//! are the three macOS window controls. It is registered as two kit theme
//! configs (dark is the primary identity); the theme follows the OS
//! appearance and switches live.

use std::rc::Rc;

use gpui_kit::component::{Theme, ThemeConfig, ThemeRegistry};
use gpui_kit::{Hsla, rgb};

/// Standard macOS traffic-light colors. These are the app's only non-grey
/// values (`plan/07 §3`); `ui/title_bar.rs` draws them.
const WINDOW_CLOSE_RGB: u32 = 0xFF5F57;
const WINDOW_MINIMIZE_RGB: u32 = 0xFEBC2E;
const WINDOW_ZOOM_RGB: u32 = 0x28C840;

/// Close, minimize, zoom — in that order.
pub fn window_control_colors() -> (Hsla, Hsla, Hsla) {
    (
        Hsla::from(rgb(WINDOW_CLOSE_RGB)),
        Hsla::from(rgb(WINDOW_MINIMIZE_RGB)),
        Hsla::from(rgb(WINDOW_ZOOM_RGB)),
    )
}

/// Dark palette (`plan/07 §3`): Warp-like near-black, low-contrast chrome,
/// greyscale accents.
const DARK_JSON: &str = r##"{
    "name": "Dislexia Dark",
    "mode": "dark",
    "radius": 6,
    "colors": {
        "background": "#0A0A0A",
        "foreground": "#E5E5E5",
        "border": "#2E2E2E",
        "input.border": "#343434",
        "ring": "#A8A8A8",
        "selection.background": "#A8A8A840",
        "caret": "#E5E5E5",
        "link.foreground": "#A8A8A8",

        "primary.background": "#686868",
        "primary.hover.background": "#717171",
        "primary.active.background": "#555555",
        "primary.foreground": "#FFFFFF",

        "secondary.background": "#202020",
        "secondary.hover.background": "#2A2A2A",
        "secondary.active.background": "#313131",
        "secondary.foreground": "#E5E5E5",

        "muted.background": "#202020",
        "muted.foreground": "#979797",

        "accent.background": "#2A2A2A",
        "accent.foreground": "#E5E5E5",

        "popover.background": "#1F1F1F",
        "popover.foreground": "#E5E5E5",

        "danger.background": "#878787",
        "danger.foreground": "#111111",
        "success.background": "#AEAEAE",
        "success.foreground": "#181818",
        "warning.background": "#B7B7B7",
        "warning.foreground": "#1D1D1D",

        "sidebar.background": "#070707",
        "sidebar.foreground": "#E5E5E5",
        "sidebar.border": "#2E2E2E",
        "sidebar.accent.background": "#151515",
        "sidebar.accent.foreground": "#E5E5E5",
        "sidebar.primary.background": "#686868",
        "sidebar.primary.foreground": "#FFFFFF",

        "title_bar.background": "#181818",
        "title_bar.border": "#2E2E2E"
    }
}"##;

/// Light counterpart (`plan/07 §3`) — a clean greyscale paper palette.
const LIGHT_JSON: &str = r##"{
    "name": "Dislexia Light",
    "mode": "light",
    "radius": 6,
    "colors": {
        "background": "#FAFAFA",
        "foreground": "#202020",
        "border": "#DFDFDF",
        "input.border": "#DFDFDF",
        "ring": "#6F6F6F",
        "selection.background": "#6F6F6F33",
        "caret": "#202020",
        "link.foreground": "#666666",

        "primary.background": "#666666",
        "primary.hover.background": "#6F6F6F",
        "primary.active.background": "#545454",
        "primary.foreground": "#FFFFFF",

        "secondary.background": "#F4F4F4",
        "secondary.hover.background": "#ECECEC",
        "secondary.active.background": "#DFDFDF",
        "secondary.foreground": "#202020",

        "muted.background": "#F4F4F4",
        "muted.foreground": "#686868",

        "accent.background": "#EDEDED",
        "accent.foreground": "#202020",

        "popover.background": "#FFFFFF",
        "popover.foreground": "#202020",

        "danger.background": "#4A4A4A",
        "danger.foreground": "#FFFFFF",
        "success.background": "#6A6A6A",
        "success.foreground": "#FFFFFF",
        "warning.background": "#707070",
        "warning.foreground": "#FFFFFF",

        "sidebar.background": "#F2F2F2",
        "sidebar.foreground": "#202020",
        "sidebar.border": "#DFDFDF",
        "sidebar.accent.background": "#ECECEC",
        "sidebar.accent.foreground": "#202020",
        "sidebar.primary.background": "#666666",
        "sidebar.primary.foreground": "#FFFFFF",

        "title_bar.background": "#FAFAFA",
        "title_bar.border": "#DFDFDF"
    }
}"##;

/// Installs the two palette configs and syncs with the OS appearance.
/// Called once from `main.rs`, after `gpui_kit::init`.
pub fn init(cx: &mut gpui_kit::App) {
    let registry = ThemeRegistry::global_mut(cx);
    for json in [LIGHT_JSON, DARK_JSON] {
        registry
            .load_themes_from_str(json)
            .expect("bundled theme JSON parses");
    }

    let light: ThemeConfig = serde_json::from_str(LIGHT_JSON).expect("light theme parses");
    let dark: ThemeConfig = serde_json::from_str(DARK_JSON).expect("dark theme parses");
    Theme::update(cx, |theme| {
        theme.light_theme = Rc::new(light);
        theme.dark_theme = Rc::new(dark);
    });

    // Follow the OS from the very first frame (`plan/07 §1`).
    Theme::sync_system_appearance(None, cx);
}

/// Re-syncs with the OS appearance; the live-switch observer calls this.
pub fn sync_system(cx: &mut gpui_kit::App) {
    Theme::sync_system_appearance(None, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_theme_json_parses() {
        for (json, mode) in [(DARK_JSON, "dark"), (LIGHT_JSON, "light")] {
            let config: ThemeConfig = serde_json::from_str(json).unwrap();
            assert_eq!(config.mode.name(), mode);
        }
    }

    #[test]
    fn bundled_themes_are_greyscale() {
        fn check(value: &serde_json::Value) {
            match value {
                serde_json::Value::String(text)
                    if text.starts_with('#') && (text.len() == 7 || text.len() == 9) =>
                {
                    let digits = text.trim_start_matches('#');
                    let red = u8::from_str_radix(&digits[0..2], 16).unwrap();
                    let green = u8::from_str_radix(&digits[2..4], 16).unwrap();
                    let blue = u8::from_str_radix(&digits[4..6], 16).unwrap();
                    assert_eq!(red, green, "{text} is not grey");
                    assert_eq!(green, blue, "{text} is not grey");
                }
                serde_json::Value::Object(map) => map.values().for_each(check),
                serde_json::Value::Array(items) => items.iter().for_each(check),
                _ => {}
            }
        }

        for json in [DARK_JSON, LIGHT_JSON] {
            let value: serde_json::Value = serde_json::from_str(json).unwrap();
            check(&value);
        }
    }
}
