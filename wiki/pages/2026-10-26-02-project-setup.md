---
title: 02 — Project setup
date: 2026-10-26
type: reference
status: reference
session_id: -
services: [build, icons, packaging]
branch: master
tickets: []
tags: [plan, setup, cargo, packaging]
related: [2026-10-26-01-architecture.md, 2026-10-26-07-theme.md, 2026-10-26-08-tray-and-lifecycle.md, 2026-10-26-09-testing.md, 2026-10-26-10-milestones.md, 2026-10-26-build-plan.md, 2026-10-26-api-notes.md, 2026-10-26-05-settings.md]
---

# 02 — Project setup

Part of the build plan — start at [[2026-10-26-build-plan]].

## TL;DR

The crate skeleton, dependency set and toolchain (stable ≥ 1.85, edition
2024), plus the two scripts that turn the tree into a launchable app:
`make-icons.sh` generates every icon from one SVG, `bundle.sh` builds the
ad-hoc-signed `.app`. App data lives under `~/Library/Application Support/dislexia/`.

---

## 1. Toolchain

```bash
rustup update stable          # stable ≥ 1.85 required (edition 2024 deps)
rustup component add rustfmt clippy
```

Optional but recommended for iteration on a GUI: `cargo install cargo-watch` (or just re-run `cargo run`).

## 2. Project bootstrap

```bash
cargo init --name dislexia
mkdir -p src/{ollama,ui,platform} assets scripts
```

## 3. `Cargo.toml`

```toml
[package]
name = "dislexia"
version = "0.2.2"
edition = "2024"
description = "macOS menu-bar chat client for local Ollama models"
license = "BSD-3-Clause"

[dependencies]
# One dependency brings: pinned GPUI (use gpui_kit::* IS GPUI),
# gpui_kit::component (Input, Textarea, Button, Dialog, Select, TextView, ...),
# default theme + assets.
gpui-kit = "0.8"

anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# HTTP to Ollama — blocking client, used on GPUI's background executor.
ureq = { version = "3", features = ["json"] }

uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# System tray + macOS platform integration.
tray-icon = "0.26"
objc2 = "0.6"
objc2-app-kit = { version = "0.3", features = [
    "NSApplication",
    "NSButton",
    "NSRunningApplication",
    "NSView",
    "NSWindow",
] }
# Native NSWindow handle, to hide the system traffic lights we draw ourselves.
raw-window-handle = "0.6"
image = { version = "0.25", default-features = false, features = ["png"] }

log = "0.4"
env_logger = "0.11"

[dev-dependencies]
tempfile = "3"

[package.metadata.bundle]   # informational; actual bundling is scripts/bundle.sh
name = "Dislexia"
identifier = "com.dislexia.app"

[profile.release]
strip = true
```

Version caveats to re-check at M0 (`cargo update` dry-run / crates.io):

- `objc2-app-kit` minor version must match the `objc2` 0.6 line used by `gpui-kit`.
- `tray-icon` 0.26 exposes its menu API (`tray_icon::menu::{Menu, MenuItem, ...}`); confirm module path at implementation time.
- If `gpui-kit` 0.8 renamed the glob re-export or `component` path, adjust imports — the contract we rely on: `use gpui_kit::*;` for GPUI and `gpui_kit::component::*` / `gpui_kit::base::*` for widgets and `ActiveTheme`.

## 4. Boot order — `main.rs`

`main.rs` holds the boot order and nothing else. Verified names are in
[[2026-10-26-api-notes]]; the shape is:

```rust
use dislexia::platform::macos::DockPolicy;
use gpui_kit::*;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    application().run(|cx| {
        gpui_kit::init(cx);         // 1. kit init: theme registry, assets (07)
        dislexia::theme::init(cx);  // 2. palettes + sync with the OS appearance (07)
        cx.bind_keys([...]);        // 3. global actions (03 §7)
        dislexia::tray::init(cx);   // 4. tray — main thread, leaks on purpose (08)
        cx.on_window_closed(|cx, _| { /* last window → Accessory */ }).detach(); // 5. (08 §4)

        let settings = dislexia::settings::load();               // 6. before the window (05)
        dislexia::ui::open_main_window(
            dislexia::ui::window_options(cx, settings.window_frame),
            settings,
            cx,
        ).expect("failed to open main window");                  // 7. window (03)
        cx.activate(true);
    });
}
```

History loads inside `Dislexia::with_settings` (a `ChatStore::load` on the root
entity), not in `main.rs`.

## 5. Running in development

```bash
cargo run                    # window app; logs to stderr via env_logger
RUST_LOG=debug cargo run
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Dev conveniences:

- Ollama must be reachable — `ollama serve` + `ollama pull llama3.2` (or any local model) before M2.
- If window/keyboard focus misbehaves during development (common for unbundled binaries on macOS), test again from the bundled `.app` (§7) before debugging code.

## 6. App data directory

```
~/Library/Application Support/dislexia/
├── settings.json
└── chats/<uuid>.json
```

Resolved once at startup (see [[2026-10-26-05-settings]] §4); create directories with `std::fs::create_dir_all`. No use of `dirs`/`directories` crate — `std::env::home_dir()` (stabilized) + the fixed suffix is enough for a macOS-only app; revisit if we ever sandbox the app.

## 7. `.app` bundle — `scripts/bundle.sh`

GPUI apps run fine unbundled, but the tray icon, proper Dock behavior and a user-facing identity want a bundle:

```bash
#!/usr/bin/env bash
set -euo pipefail
APP=Dislexia
VERSION=$(python3 -c 'import re,sys; print(re.search(r"^version = \"(.*)\"", open("Cargo.toml").read(), re.M).group(1))')
BUILD="${BINARY:-target/release/dislexia}"   # CI passes a cross-target path
BUNDLE="${BUNDLE_DIR:-target}/${APP}.app"

# regenerate the icon when media/dislexia.svg is newer than assets/icon.icns
[ -f assets/icon.icns ] && [ assets/icon.icns -nt media/dislexia.svg ] || scripts/make-icons.sh

mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"
cp "$BUILD" "$BUNDLE/Contents/MacOS/${APP}"
cp assets/icon.icns "$BUNDLE/Contents/Resources/"
# Info.plist: CFBundleShortVersionString = $VERSION, no LSUIElement
codesign --force --deep --sign - "$BUNDLE"   # ad-hoc; local use
```

- The version comes from `Cargo.toml`, so a release is a version bump — never a hand-edited plist.
- `BINARY` / `BUNDLE_DIR` exist for CI: `.github/scripts/package-app.sh` points them at
  `target/<triple>/release/dislexia` and a staging dir, then zips `Dislexia.app` + `README.md`
  into `dist/`.
- The icon is regenerated when the SVG is newer, so a stale checked-in `.icns` cannot ship.

Usage: `cargo build --release && scripts/bundle.sh && open target/Dislexia.app`.

## 8. Icon pipeline — `scripts/make-icons.sh` (created in M0)

The app's icon is the hand-drawn spiral stroke at [`media/dislexia.svg`](../../media/dislexia.svg) (provenance and rules in [`media/README.md`](../../media/README.md)). All icon assets are **generated** from that one SVG; nothing under `assets/` is hand-edited:

```
media/dislexia.svg
  ├─ svg-glyph.py (black ink, square canvas, margin) ─┬─ app variant (9% margin)
  │                                                   │    → rasterize 1024px → png-template.py
  │                                                   │    → iconset/{16,32,128,256,512}{,@2x}.png → iconutil → assets/icon.icns
  │                                                   └─ tray variant (6% margin, 1.25× stroke)
  │                                                        → rasterize 1024px → png-template.py
  │                                                        → sips 16px + 32px → assets/tray-16@1x.png, tray-32@2x.png
  └─ both variants are black ink on transparency
```

- The source art is white ink on a non-square transparent canvas, drawn edge to edge. `scripts/svg-glyph.py` normalizes it before rasterizing: white `fill`/`stroke` values become black, the artwork is centred on a square canvas with a margin, and `--stroke-weight` optically bolds the glyph. Two variants come out of it — the app icon wants a macOS-sized margin, the tray glyph wants every pixel it can get at 16px.
- Rasterization on a dev Mac: `qlmanage -t -s <size>` or `rsvg-convert` if installed; `sips` handles PNG resizes.
- `qlmanage` draws on an opaque white canvas. `scripts/png-template.py` rewrites each raster as pure black with alpha derived from the ink, which is what a template image needs — and what keeps the Dock icon from being a white square. If the rasterizer already produced transparency, the script keeps that alpha.
- Tray PNGs are **template images** (loaded with `set_template(true)` — [[2026-10-26-08-tray-and-lifecycle]] §1, [[2026-10-26-07-theme]] §5) so macOS adapts them to light/dark menu bars automatically.
- `bundle.sh` (§7) calls `make-icons.sh` when `assets/` is out of date; the tray code at runtime reads the two tray PNGs via `image::load_from_memory` (embedded with `include_bytes!`).

Do **not** bake colors into any derived PNG except opaque black — template rendering and the zcode mono aesthetic both depend on single-color glyphs.

## 9. Repository hygiene

- `.gitignore`: `/target`, `*.app`.
- **`README.md`** (written in M7) — the repo/app face. Required contents:
  - one-paragraph pitch: writing-fixer chat app for a local Ollama server + a screenshot of the main window (and one of the tray menu)
  - feature list: streaming chat, default text-fixer prompt (link `prompts/default-system-prompt.md`), chat history, settings, tray/background mode, OS light/dark theme
  - requirements: macOS 13+ (confirm the GPUI Kit minimum at M0), Rust stable ≥ 1.85, local Ollama (`ollama serve` + at least one pulled model)
  - dev quickstart: `cargo run` next to the `ollama serve` / `ollama pull` one-liners
  - build & package: `scripts/make-icons.sh` + `scripts/bundle.sh` → `target/Dislexia.app`
  - where data lives: `~/Library/Application Support/dislexia/` (settings.json, chats/)
  - credits & license: the `media/dislexia.svg` icon (project artwork), [gpui-kit](https://gpui-kit.com), project license
  - pointer to `wiki/pages/2026-10-26-build-plan.md` and the rest of the build-plan pages for architecture and milestones
- Keep the build-plan pages in the wiki — update them when reality diverges (they are the implementation contract).
- No workspace split needed; single crate with modules.

---

## Follow-ups

- The repo `README.md` screenshot is still pending (§9).
- The bundled `.app` has not had its interactive QA pass yet — see [[2026-10-26-10-milestones]] M7.

## References

- Related: [[2026-10-26-01-architecture]], [[2026-10-26-07-theme]],
  [[2026-10-26-08-tray-and-lifecycle]], [[2026-10-26-09-testing]],
  [[2026-10-26-10-milestones]], [[2026-10-26-build-plan]], [[2026-10-26-api-notes]],
  [[2026-10-26-05-settings]]
- Code: `Cargo.toml`, `scripts/bundle.sh`, `scripts/make-icons.sh`, `scripts/svg-glyph.py`, `scripts/png-template.py`, `Makefile`
