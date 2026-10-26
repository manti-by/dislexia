# 07 — Theme (OS light/dark, zcode-like palette)

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

## 1. Principle

The app follows the **OS appearance** — there is no in-app light/dark toggle. Colors are never hard-coded in views: every view reads the GPUI Kit theme (`ActiveTheme`), and this doc defines the only place colors are assigned.

## 2. Appearance tracking

GPUI exposes window appearance APIs (verify exact signatures against the pinned version in `gpui-kit`):

- `cx.window_appearance() -> WindowAppearance` — query at render time
- `cx.observe_window_appearance(|this, appearance, cx| ...)` — callback on system appearance change, fires at launch and on live switches

Wiring in `src/theme.rs`:

```rust
pub fn init(cx: &mut App) {
    // 1. install default kit theme first (component::init)
    // 2. apply palette for the current appearance
    cx.observe_window_appearance(|_this, appearance, cx| {
        apply_palette(appearance, cx);
    }).detach();
}

fn apply_palette(appearance: WindowAppearance, cx: &mut App) {
    let dark = appearance.is_dark();
    // mutate the kit's ActiveTheme: background, surface, border, accent, text colors…
    cx.notify();
}
```

This yields live switching when the user changes the macOS appearance, including auto mode (macOS notifies per-window; GPUI re-calls the observer).

## 3. Palette — zcode-like

Dark is the primary identity (zcode is a dark terminal); light is a clean counterpart. Values are starting points — fine-tune during M5 against a real screenshot of zcode.

| Role | Dark | Light | Used for |
|---|---|---|---|
| `background` | `#0A0A0A` | `#FAFAFA` | App/window background |
| `surface` | `#202020` | `#F4F4F4` | Secondary panels, input field bg |
| `sidebar` | `#070707` | `#F2F2F2` | Sidebar and user message block |
| `border` | `#2E2E2E` | `#DFDFDF` | Hairlines, input border |
| `text` | `#E5E5E5` | `#202020` | Primary text |
| `text_muted` | `#979797` | `#686868` | Meta lines, placeholders, timestamps |
| `accent` | `#686868` | `#666666` | Send button, assistant bar, active-row bar |
| `accent_hover` | `#717171` | `#6F6F6F` | |
| `ring` / `link` | `#A8A8A8` | `#6F6F6F` / `#666666` | Focus ring, links, selection |
| `error` | `#878787` | `#4A4A4A` | Error blocks, connection dot (down) |
| `success` | `#AEAEAE` | `#6A6A6A` | Connection dot (up), done state |
| `warning` | `#B7B7B7` | `#707070` | Stopped state, "(missing on server)" |

Zcode-ish signature details (dark): near-black monochrome chrome based on Warp's default dark theme (`#181818` / `#d8d8d8`) with the user's darker near-black base (`#0A0A0A`), low-contrast panels (the sidebar is a step darker still), greyscale accents, monospace for all meta (timestamps, model names, group headers).

The palette is **greyscale**: every theme color has R = G = B. The only chromatic values in the app are the three macOS window controls — close `#FF5F57`, minimize `#FEBC2E`, zoom `#28C840` — defined in `src/theme.rs` and drawn by `src/ui/title_bar.rs`.

## 4. How the palette is applied

- GPUI Kit's theme object is mutated with the roles above so **all stock components** (Input, Button, Modal, List, Dropdown, TextArea) restyle automatically — that's the "no customization" requirement.
- The only custom-styled primitives: the assistant message's 2px left bar (`accent`), the streaming cursor `▌` (`accent`), sidebar active item (3px `accent` bar + `surface` row bg).

## 5. Tray icon & appearance

- Tray icons are **template images** (pure black + alpha, named e.g. `tray-16@1x.png`/`tray-32@2x.png`, `image::load_from_memory` at init). macOS renders templates to match the menu bar in both appearances — no manual dark/light icon swap, no `is_dark` branching in tray code.
- The PNGs are generated from the single icon source [`media/icon-typewriter.svg`](../media/icon-typewriter.svg) by `scripts/make-icons.sh` ([02](02-project-setup.md) §8) — opaque-black typewriter glyph on transparency, nothing else.
- Mark the image as template (`set_template(true)` on the icon builder) — critical, otherwise the icon is invisible in one of the modes.

## 6. Contrast checks (M5 acceptance)

- All text/background pairs meet WCAG AA (≥ 4.5:1) in **both** palettes; verify `text_muted` on `background` explicitly (dim text is the usual offender; the values above are ~6.8:1 dark and ~5.3:1 light).
- Focus rings visible in both appearances (accent on background).
