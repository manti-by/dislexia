//! OS appearance tracking + the only place colors are assigned
//! (`plan/07-theme.md`). The zcode-like palette is registered as two kit
//! theme configs (dark is the primary identity); the theme follows the OS
//! appearance and switches live.

use std::rc::Rc;

use gpui_kit::component::{Theme, ThemeConfig, ThemeRegistry};

/// Dark palette (`plan/07 §3`): near-black blue-leaning background,
/// low-contrast chrome, one blue accent used sparingly.
const DARK_JSON: &str = r##"{
    "name": "Dislexia Dark",
    "mode": "dark",
    "radius": 6,
    "colors": {
        "background": "#1A1B23",
        "foreground": "#E6E6EF",
        "border": "#2E303C",
        "input.border": "#2E303C",
        "ring": "#5B8CFF",
        "selection.background": "#5B8CFF44",
        "caret": "#E6E6EF",
        "link.foreground": "#5B8CFF",

        "primary.background": "#5B8CFF",
        "primary.hover.background": "#7AA3FF",
        "primary.active.background": "#527CE8",
        "primary.foreground": "#FFFFFF",

        "secondary.background": "#22232D",
        "secondary.hover.background": "#2A2B36",
        "secondary.active.background": "#2E303C",
        "secondary.foreground": "#E6E6EF",

        "muted.background": "#22232D",
        "muted.foreground": "#8B8D9E",

        "accent.background": "#2A2B36",
        "accent.foreground": "#E6E6EF",

        "popover.background": "#22232D",
        "popover.foreground": "#E6E6EF",

        "danger.background": "#E5546B",
        "danger.foreground": "#FFFFFF",
        "success.background": "#4CC38A",
        "success.foreground": "#0F1A14",
        "warning.background": "#E0AF68",
        "warning.foreground": "#241C0D",

        "sidebar.background": "#22232D",
        "sidebar.foreground": "#E6E6EF",
        "sidebar.border": "#2E303C",
        "sidebar.accent.background": "#2A2B36",
        "sidebar.accent.foreground": "#E6E6EF",
        "sidebar.primary.background": "#5B8CFF",
        "sidebar.primary.foreground": "#FFFFFF",

        "title_bar.background": "#1A1B23",
        "title_bar.border": "#2E303C"
    }
}"##;

/// Light counterpart (`plan/07 §3`) — a clean paper-like palette.
const LIGHT_JSON: &str = r##"{
    "name": "Dislexia Light",
    "mode": "light",
    "radius": 6,
    "colors": {
        "background": "#FAFAF7",
        "foreground": "#1F2024",
        "border": "#DDDDD5",
        "input.border": "#DDDDD5",
        "ring": "#3E6BFF",
        "selection.background": "#3E6BFF33",
        "caret": "#1F2024",
        "link.foreground": "#3E6BFF",

        "primary.background": "#3E6BFF",
        "primary.hover.background": "#5C85FF",
        "primary.active.background": "#3557E0",
        "primary.foreground": "#FFFFFF",

        "secondary.background": "#F0F0EB",
        "secondary.hover.background": "#E6E6DF",
        "secondary.active.background": "#DDDDD5",
        "secondary.foreground": "#1F2024",

        "muted.background": "#F0F0EB",
        "muted.foreground": "#6E7079",

        "accent.background": "#E8E8E1",
        "accent.foreground": "#1F2024",

        "popover.background": "#FFFFFF",
        "popover.foreground": "#1F2024",

        "danger.background": "#C8354F",
        "danger.foreground": "#FFFFFF",
        "success.background": "#2E9E6B",
        "success.foreground": "#F2FAF6",
        "warning.background": "#B07C2A",
        "warning.foreground": "#FFF9EC",

        "sidebar.background": "#F0F0EB",
        "sidebar.foreground": "#1F2024",
        "sidebar.border": "#DDDDD5",
        "sidebar.accent.background": "#E4E4DC",
        "sidebar.accent.foreground": "#1F2024",
        "sidebar.primary.background": "#3E6BFF",
        "sidebar.primary.foreground": "#FFFFFF",

        "title_bar.background": "#FAFAF7",
        "title_bar.border": "#DDDDD5"
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
}
