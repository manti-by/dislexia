---
title: Spiral icons and window frame persistence
date: 2026-10-26
type: implementation
status: resolved
session_id: ses_e867f0de3ffeQiOVdSgso6mEaV
services: [icon-pipeline, window, settings]
branch: master
tickets: []
tags: [icons, svg, tray, dock, window, settings, persistence, gpui]
related: [2026-10-26-02-project-setup.md, 2026-10-26-03-ui-layout.md, 2026-10-26-05-settings.md, 2026-10-26-07-theme.md]
---

# Spiral icons and window frame persistence

## TL;DR

Replaced the typewriter mark with the new `media/dislexia.svg` spiral for both
the Dock and tray icons, which required a new normalization step: the art is
white ink on a non-square transparent canvas, so `qlmanage` rasterized it to a
blank white square. `scripts/svg-glyph.py` now turns it into a black-ink,
square, padded glyph, and the ink → alpha pass now also applies to the app icon
(which was previously a black glyph on an opaque white square in the Dock).
Separately, the main window remembers its position and size across launches,
per display, via a new `window_frame` settings field.

---

## Overview

Two independent changes, both finished and gated by `make check`:

- **Icons** — `media/dislexia.svg` is the only icon source; `media/icon-typewriter.svg`
  and `media/icon-preview-512.png` are deleted. All references (Makefile, README,
  AGENTS.md, `media/README.md`, `bundle.sh`, `package-app.sh`, plan pages 02/07/08/10,
  build-plan) point at the new file.
- **Window frame** — `settings::WindowFrame` persists position, size and display
  UUID; `ui::window_options` restores it with clamping; the root entity observes
  window bounds and rides the existing debounced settings write.

## Step 1 — Icon source could not be rasterized verbatim

**Symptom.** `qlmanage -t -s 512 media/dislexia.svg` produces a 512×512 image
whose every pixel is `(255, 255, 255, 255)` — an opaque *white* canvas with
*white* ink, i.e. a blank square. `rsvg-convert` is not installed on this
machine, so `qlmanage` is the live path in `scripts/make-icons.sh`.

Three properties of the art fought the pipeline:

- ink is `stroke="white"` — invisible against the rasterizer's white canvas;
- the canvas is 265×278, not square, and the iconset and tray sizes are square;
- the stroke runs to the frame edges (the path is clipped by its own viewBox),
  so there is no margin at all.

**Fix.** New helper `scripts/svg-glyph.py` emits an equivalent square SVG:

- `WHITE_INK` rewrites white-ish `fill`/`stroke` values to `#000000`
  (`scripts/svg-glyph.py:31`, applied at `:104`);
- the root's presentation attributes are re-emitted on a wrapper `<g>` so a
  `fill="none"` source stays an outline instead of becoming a solid blob
  (`scripts/svg-glyph.py:120`);
- the artwork is scaled into a centred square canvas with `--padding`
  (`scripts/svg-glyph.py:92`);
- `--stroke-weight` multiplies every `stroke-width` for optical sizing
  (`scripts/svg-glyph.py:107`).

**Rejected alternative.** Composing the white art onto a dark rounded-square
canvas. It preserves the artwork exactly but bakes in a dark appearance, which
clashes with macOS icon appearance modes; black-on-transparent was chosen
instead and keeps the existing monochrome-glyph identity.

## Step 2 — One master per icon, two optical variants

**File:** `scripts/make-icons.sh:24` — the `rasterize` helper takes
`name padding weight` and produces a transparent 1024px master with black ink:
`svg-glyph.py` → `qlmanage`/`rsvg-convert` → `png-template.py`.

Two variants (`scripts/make-icons.sh:46`):

- `rasterize app 0.09 1.0` — macOS-sized margin, original stroke, feeds the
  iconset (`16,32,128,256,512` plus `@2x`) → `iconutil` → `assets/icon.icns`.
- `rasterize tray 0.06 1.25` — tight margin and 1.25× stroke, because at 16px
  the spiral is a hairline: measured max alpha at 16px was 174/255 before the
  boost. `sips` downsizes it to `tray-16@1x.png` and `tray-32@2x.png`.

**Behaviour change worth noting.** Ink → alpha now runs for the app icon too.
Before, the `.icns` was built straight from the opaque-white `qlmanage` output,
so the Dock icon was a black glyph on a white square. It is now black ink on
transparency, and `qlmanage`'s canvas no longer leaks into the bundle.

Regenerating twice produces byte-identical assets (checked with `shasum`), so
`assets/` diffs stay meaningful in git.

## Step 3 — Settings gained a window frame

**File:** `src/settings.rs:32` — `WindowFrame { x, y, width, height, display }`,
with `display: Option<Uuid>` holding `CGDisplayCreateUUIDFromDisplayID`, which is
stable across restarts. `Settings::window_frame` is `Option<…>`, `serde(default)`,
so an existing `settings.json` loads unchanged (`src/settings.rs:24`).

Coordinates are relative to the display the window was on — that is how GPUI's
macOS backend reports and applies bounds — which is exactly why the display UUID
is persisted alongside them. A frame whose display is gone falls back to the
centred default rather than being restored onto the wrong screen.

## Step 4 — Restore with clamping

**File:** `src/ui/mod.rs:44` — `window_options(cx, frame)` looks the display up
by UUID and sets `options.display_id`, so the window opens on the right monitor.

**File:** `src/ui/mod.rs:75` — `fit_frame` clamps a stale frame onto its display:

- never smaller than the 480×360 minimum;
- never larger than the screen;
- at least `MIN_VISIBLE` (80) points inside the display on both axes, so the
  title bar stays grabbable after a monitor is unplugged or a resolution
  changes.

`min`/`max` are ordered to never panic on a degenerate display, and NaN inputs
degrade to the defaults.

## Step 5 — Saving rides the existing debounce

**File:** `src/app.rs:209` — `cx.observe_window_bounds` is subscribed alongside
the activation observer. GPUI fires it for both `on_moved` and `on_resize`.

**File:** `src/app.rs:439` — `remember_window_frame` reads `window.bounds()` plus
`window.display(cx)?.uuid()` and, only when the frame actually changed, calls
the existing `schedule_save` (300 ms debounce), so a drag writes once.

Native fullscreen reports the *screen* rather than the frame the user picked
(`WindowBounds::Fullscreen`), so it is skipped and the last windowed frame
survives. `main.rs:42` and `app.rs:107` (tray recreating the window) both pass
`settings.window_frame` into `window_options`.

## Test Results

`make check` green: `fmt --check`, clippy warning-free, 42 unit tests and 9
mock-Ollama integration tests pass.

New tests:

- `settings::tests::window_frame_round_trips_and_is_optional` — serde round
  trip, plus a legacy `settings.json` without the field loading as `None`.
- `settings::tests::defaults_match_the_plan` — asserts `window_frame` is `None`.
- `ui::tests::saved_frame_is_restored_verbatim`
- `ui::tests::tiny_frame_is_grown_to_the_minimum_size`
- `ui::tests::oversized_frame_is_shrunk_to_the_display`
- `ui::tests::frame_off_the_right_edge_keeps_a_grabbable_column_visible`
- `ui::tests::frame_off_the_bottom_edge_keeps_its_title_bar_visible`
- `ui::tests::frame_above_the_top_edge_is_pushed_down`
- `ui::tests::negative_origin_from_a_second_display_is_kept`

Manual checks: a 5-second smoke launch (`target/debug/dislexia`) started without
a panic and wrote `window_frame` for the first time; icon renders were compared
visually at 16/32/128/256px on light and dark backgrounds.

---

## Follow-ups

- At 16px @1x the tray spiral is still delicate by nature (it is a hairline
  line drawing); retina uses the 32px asset and reads clearly. `--stroke-weight`
  in `scripts/svg-glyph.py` is the knob if it needs to be heavier.
- The source art is clipped at its own viewBox edges (strokes run off the
  canvas by design). If that was not intentional, the fix belongs in
  `media/dislexia.svg`, not in the pipeline.
- `make-icons.sh` falls back to `qlmanage`; nothing exercises the
  `rsvg-convert` branch on this machine.
- Window frame is windowed-state only: macOS never reports `Maximized` through
  GPUI's `window_bounds()`, so a zoomed window restores at its zoomed size but
  not as a zoomed state.

## References

- Related: [[2026-10-26-02-project-setup]] §8 (icon pipeline),
  [[2026-10-26-03-ui-layout]] §6 (sizing & window),
  [[2026-10-26-05-settings]] §1 (`window_frame`),
  [[2026-10-26-07-theme]] §5 (tray template images)
- Files: `scripts/svg-glyph.py`, `scripts/make-icons.sh`, `scripts/png-template.py`,
  `src/settings.rs`, `src/ui/mod.rs`, `src/app.rs`, `src/main.rs`,
  `media/dislexia.svg`, `media/README.md`
