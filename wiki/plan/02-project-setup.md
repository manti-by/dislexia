# 02 — Project setup

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

## 1. Toolchain

```bash
rustup update stable          # stable ≥ 1.85 required (edition 2024 deps)
rustup component add rustfmt clippy
```

Optional but recommended for iteration on a GUI: `cargo install cargo-watch` (or just re-run `cargo run`).

## 2. Project bootstrap

```bash
cargo init --name dislexia
mkdir -p src/{ollama,ui,platform} assets scripts plan
```

## 3. `Cargo.toml`

```toml
[package]
name = "dislexia"
version = "0.1.0"
edition = "2024"
description = "macOS menu-bar chat client for local Ollama models"

[dependencies]
# One dependency brings: pinned GPUI (use gpui_kit::* IS gpui),
# gpui_kit::component (Input, TextArea, Button, List, Modal, TitleBar, Dropdown,
# Markdown, ...), default theme + assets.
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
    "NSRunningApplication",
    "NSWindow",
] }
image = { version = "0.25", default-features = false, features = ["png"] }

log = "0.4"
env_logger = "0.11"

[dev-dependencies]
tempfile = "3"

[package.metadata.bundle]   # informational; actual bundling is scripts/bundle.sh
name = "Dislexia"
identifier = "com.dislexia.app"
```

Version caveats to re-check at M0 (`cargo update` dry-run / crates.io):

- `objc2-app-kit` minor version must match the `objc2` 0.6 line used by `gpui-kit`.
- `tray-icon` 0.26 exposes its menu API (`tray_icon::menu::{Menu, MenuItem, ...}`); confirm module path at implementation time.
- If `gpui-kit` 0.8 renamed the glob re-export or `component` path, adjust imports — the contract we rely on: `use gpui_kit::*;` for GPUI and `gpui_kit::component::*` / `gpui_kit::base::*` for widgets and `ActiveTheme`.

## 4. Minimal `main.rs` (M0 target)

Sketch — verify API names against the pinned GPUI:

```rust
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme, Root};

fn main() {
    env_logger::init();
    Application::new().run(|cx: &mut App| {
        // 1. theme init (07-theme.md)
        // 2. settings + history load (05, 06)
        // 3. tray init — must happen on main thread (08-tray-and-lifecycle.md)
        // 4. open main window:
        let opts = WindowOptions::default(); // windowed, default size 1000x680
        cx.open_window(opts, |window, cx| {
            window.activate_window();
            dislexia::ui::workspace(cx)      // returns Entity implementing Render
        })
        .expect("failed to open window");
        cx.activate(true);
        // 5. bind global keys + actions (03-ui-layout.md §7)
    });
}
```

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

Resolved once at startup (see [05-settings.md](05-settings.md) §4); create directories with `std::fs::create_dir_all`. No use of `dirs`/`directories` crate — `std::env::home_dir()` (stabilized) + the fixed suffix is enough for a macOS-only app; revisit if we ever sandbox the app.

## 7. `.app` bundle — `scripts/bundle.sh`

GPUI apps run fine unbundled, but the tray icon, proper Dock behavior and a user-facing identity want a bundle:

```bash
#!/usr/bin/env bash
set -euo pipefail
APP=Dislexia
BUILD=target/release/dislexia
rm -rf "target/${APP}.app"
mkdir -p "${APP}.app/Contents/"{MacOS,Resources}
cp "$BUILD" "${APP}.app/Contents/MacOS/${APP}"
cp assets/icon.icns "${APP}.app/Contents/Resources/"
cat > "${APP}.app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>              <string>Dislexia</string>
  <key>CFBundleIdentifier</key>        <string>com.dislexia.app</string>
  <key>CFBundleExecutable</key>        <string>Dislexia</string>
  <key>CFBundlePackageType</key>       <string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key>           <string>1</string>
  <key>CFBundleIconFile</key>          <string>icon</string>
  <key>NSHighResolutionCapable</key>   <true/>
  <!-- NOTE: no LSUIElement here — Dock visibility is toggled at runtime (08 §4) -->
</dict>
</plist>
PLIST
codesign --force --deep --sign - "${APP}.app"   # ad-hoc; local use
echo "Built ${APP}.app"
```

Usage: `cargo build --release && scripts/bundle.sh && open target/Dislexia.app`.

## 8. Icon pipeline — `scripts/make-icons.sh` (created in M0)

The app's icon is the minimal monochrome **typewriter** glyph at [`media/icon-typewriter.svg`](../media/icon-typewriter.svg) (Material Design Icons, Apache-2.0 — provenance and rules in [`media/README.md`](../media/README.md)). All icon assets are **generated** from that one SVG; nothing under `assets/` is hand-edited:

```
media/icon-typewriter.svg
  ├─ rasterize 1024px → icon.iconset/{16,32,64,128,256,512,1024}{,@2x}.png → iconutil → assets/icon.icns
  └─ rasterize 16px + 32px (black glyph, transparent bg)      → assets/tray-16@1x.png, tray-32@2x.png
```

- Rasterization on a dev Mac: `qlmanage -t -s <size>` or `rsvg-convert` if installed; `sips` handles PNG resizes.
- Tray PNGs are **template images** (loaded with `set_template(true)` — [08](08-tray-and-lifecycle.md) §1, [07](07-theme.md) §5) so macOS adapts them to light/dark menu bars automatically.
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
  - credits & license: MDI "typewriter" icon (Apache-2.0), [gpui-kit](https://gpui-kit.com), project license
  - pointer to `BUILD_PLAN.md` and `plan/` for architecture and milestones
- Keep `plan/` in the repo — update it when reality diverges (it is the implementation contract).
- No workspace split needed; single crate with modules.
