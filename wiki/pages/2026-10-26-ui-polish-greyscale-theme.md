---
title: UI polish: greyscale theme, resizable sidebar, tray icon
date: 2026-10-26
type: implementation
status: resolved
session_id: ses_e87bc3500ffeqWwxWU1LwJ2DbX
services: [ui, tray, theme]
branch: master
tickets: []
tags: [ui, theme, tray, gpui, layout]
related: [2026-10-26-03-ui-layout.md, 2026-10-26-07-theme.md, 2026-10-26-08-tray-and-lifecycle.md, 2026-10-26-02-project-setup.md, 2026-10-26-10-milestones.md, 2026-10-26-spiral-icons-window-frame.md]
---

# UI polish: greyscale theme, resizable sidebar, tray icon

## TL;DR

Fixed the window chrome, text wrapping, tray icon and default sizing, then
replaced the green-tinted palette with a near-black Warp-style greyscale theme.
The session list is now a draggable 150–420px split panel, the composer stays
pinned to the window bottom, and only the macOS close/minimize/zoom controls
keep color. `make check` is green.

---

## Overview

The work started from a list of visual defects: oversized window controls, a
fixed-width chat list, markdown and user text running past the window edge, a
600px window, and a tray icon rendered as a white square. Later iterations
changed the default window size to 800×600, shifted the palette to green, then
toned it down to greyscale with only the traffic lights colored.

The implementation stays inside the existing module boundaries: `src/ui/` only
renders, `src/platform/macos.rs` owns AppKit calls, `src/theme.rs` owns every
color value, and `src/ollama/` remains untouched. The code changes are committed
as `1b5cee9` (`Update app theme`).

## Step 1 — Window chrome and 12px controls

macOS 27 draws its native traffic lights at 14px and AppKit has no supported
API to scale them; resizing the `NSButton` frames does not scale the drawn
circle. The fix is to hide the native buttons and draw our own:

- **File:** `src/platform/macos.rs:46` — `hide_window_buttons` resolves the
  window's `NSView` through `raw-window-handle`, gets the `NSWindow`, and marks
  close/minimize/zoom hidden.
- **File:** `src/ui/mod.rs:40` — `window_options` clears
  `traffic_light_position` so GPUI does not reposition hidden AppKit buttons.
- **File:** `src/ui/title_bar.rs` — three 12px circles, 8px apart, at the
  title bar's leading edge. Each control stops mouse-down propagation so the
  title bar's drag handler does not swallow clicks, and maps to
  `remove_window`, `minimize_window` and `zoom_window` respectively.
- The controls show the native glyphs on hover and dim to neutral grey while
  the window is inactive (`observe_window_activation` in `src/app.rs`).
- The app title was reduced to 12px and the title bar's left padding was
  reduced to 12px to match the custom controls.

## Step 2 — Default size and layout

- **File:** `src/ui/mod.rs:22` — default window is **800×600**, minimum
  **480×360**.
- **File:** `src/ui/sidebar.rs:17` — sidebar defaults to **150px**, can be
  dragged from **150px to 420px**.
- **File:** `src/app.rs:842` — the sidebar and chat pane are an
  `h_resizable("main-split")` group. The sidebar panel is `fixed()` so it keeps
  its width when the window resizes; the chat panel takes the remaining space.
- **File:** `src/ui/chat_pane.rs:87` — the chat pane is `h_full()`, so its
  composer is anchored to the window bottom. The transcript wrapper grows above
  it.

## Step 3 — Text wrapping and chat typography

The root cause was layout sizing, not markdown: the transcript column was sized
from its widest line (`max_w + mx_auto` without a definite width), and the user
bubble's `min_w_0()` let flex shrink collapse it to a narrow column at small
window widths.

- **File:** `src/ui/chat_pane.rs:28` — the transcript content is `w_full()`
  with a 16px inset; the scroll container also takes the full pane width.
- **File:** `src/ui/message.rs` — assistant/error bodies are `w_full()`, and
  the user bubble is `w_full().max_w(520px)`. The `min_w_0()` that collapsed the
  bubble was removed.
- **File:** `src/app.rs:178` — the composer placeholder was shortened so it
  fits the narrower window.
- **File:** `src/ui/message.rs:57` — chat body text is **13px** (user,
  assistant, error and empty hint), with 11px mono metadata unchanged.

## Step 4 — Tray icon alpha

The checked-in tray PNGs had alpha 255 on every pixel: `qlmanage` rasterized
the SVG onto an opaque white canvas, so template rendering painted a solid
white square.

- **File:** `scripts/png-template.py` — stdlib PNG decoder that converts an
  opaque white-background raster to a black glyph whose alpha is
  `255 - luminance`; if the source already has transparency, it keeps it.
- **File:** `scripts/make-icons.sh` — resizes the master with `sips`, then
  pipes each tray size through `png-template.py` before writing `assets/`.
- Regenerated `assets/tray-16@1x.png` and `assets/tray-32@2x.png`; the tray now
  shows the typewriter glyph. The other white square still visible in the menu
  bar belongs to a different app.

## Step 5 — Greyscale theme

All theme colors now have `R = G = B`. The dark palette follows Warp's default
dark neutral range (`#181818` / `#d8d8d8`) with a darker near-black base; the
light palette is a neutral paper scale.

Key dark values:

- background `#0A0A0A`; sidebar `#070707`; title bar `#181818`
- foreground `#E5E5E5`; muted `#979797`
- border `#2E2E2E`; input border `#343434`
- accent (Send, assistant bar, active row) `#686868`
- danger/success/warning `#878787` / `#AEAEAE` / `#B7B7B7`

Key light values:

- background `#FAFAFA`; sidebar `#F2F2F2`
- foreground `#202020`; muted `#686868`
- border `#DFDFDF`
- accent `#666666`
- danger/success/warning `#4A4A4A` / `#6A6A6A` / `#707070`

The only chromatic values are the window controls, defined in `src/theme.rs:14`
and drawn by `src/ui/title_bar.rs`:

- close `#FF5F57`
- minimize `#FEBC2E`
- zoom `#28C840`

`src/theme.rs:175` adds `bundled_themes_are_greyscale`, which walks both theme
JSON blobs and fails if any hex color has unequal channels. This keeps the
greyscale rule from regressing when the palette is edited.

## Files modified

- `src/ui/title_bar.rs` — new custom window controls.
- `src/platform/macos.rs` — native traffic-light hiding.
- `src/ui/mod.rs` — default/minimum window sizes and window options.
- `src/app.rs` — resizable split, native-button hide call, activation repaint,
  12px app title.
- `src/ui/sidebar.rs` — resizable panel width constants; full-width sidebar.
- `src/ui/chat_pane.rs` — full-width transcript, bottom-anchored composer.
- `src/ui/message.rs` — message widths and 13px body text.
- `src/theme.rs` — greyscale palettes, traffic-light constants, greyscale test.
- `scripts/make-icons.sh` and `scripts/png-template.py` — tray alpha pipeline.
- `assets/tray-16@1x.png`, `assets/tray-32@2x.png` — regenerated with alpha.
- `Cargo.toml`, `Cargo.lock` — `raw-window-handle`, `NSButton`/`NSView`.
- `AGENTS.md`, `media/README.md`, [[2026-10-26-02-project-setup]],
  [[2026-10-26-03-ui-layout]], [[2026-10-26-07-theme]],
  [[2026-10-26-10-milestones]] — docs synced to the new behavior.

## Test Results

- `cargo build` — clean.
- `cargo clippy --all-targets` — warning-free.
- `cargo fmt --check` — clean.
- `cargo test` — **34** library tests and **9** mock-Ollama integration tests
  pass, including the new `theme::tests::bundled_themes_are_greyscale`.
- Manual visual verification on macOS 27: 800×600 window, 12px controls, 150px
  draggable sidebar, wrapped message text, bottom-pinned composer, greyscale UI,
  typewriter tray icon.
- `rg '#[0-9a-fA-F]{6}' src` — no colors outside `src/theme.rs`.

## Follow-ups

- Persist the sidebar width across window recreation/app restarts; the current
  split state is window-keyed and resets to 150px.
- Superseded: the typewriter tray glyph recorded here was replaced by the
  `media/dislexia.svg` spiral in [[2026-10-26-spiral-icons-window-frame]].
- The resizable sidebar's divider is a 1px hairline; it has a hover pill and
  cursor from `h_resizable`, but it is intentionally quiet at rest.
- Native traffic lights are hidden once per window. If AppKit recreates them
  after fullscreen transitions, both sets could show; worth checking if
  fullscreen becomes a supported workflow.
- The settings modal is 560px wide while the minimum window is 480px; the modal
  can be wider than a heavily shrunk window.

## References

- Related: [[2026-10-26-03-ui-layout]], [[2026-10-26-07-theme]],
  [[2026-10-26-08-tray-and-lifecycle]], [[2026-10-26-02-project-setup]],
  [[2026-10-26-10-milestones]], [[2026-10-26-spiral-icons-window-frame]]
- Plan: [[2026-10-26-03-ui-layout]], [[2026-10-26-07-theme]],
  [[2026-10-26-08-tray-and-lifecycle]]
- Code: `src/ui/title_bar.rs`, `src/ui/chat_pane.rs`, `src/theme.rs`
- External: none
