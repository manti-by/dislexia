---
title: 08 — System tray & app lifecycle
date: 2026-10-26
type: reference
status: reference
session_id: -
services: [tray, platform]
branch: master
tickets: []
tags: [plan, tray, lifecycle, macos]
related: [2026-10-26-02-project-setup.md, 2026-10-26-07-theme.md, 2026-10-26-09-testing.md, 2026-10-26-10-milestones.md, 2026-10-26-build-plan.md, 2026-10-26-06-chat-history.md, 2026-10-26-05-settings.md]
---

# 08 — System tray & app lifecycle

Part of the build plan — start at [[2026-10-26-build-plan]].

## TL;DR

Dislexia is a menu-bar utility: the tray icon is created on the main thread
and never dropped, closing the window destroys it and flips the Dock policy to
Accessory, and the app keeps running until an explicit Quit. Also records the M6
findings on the main-thread hop and on the auto-quit risk.

---

Goal: Dislexia behaves like a menu-bar utility — **always in the tray**, survives window close as a background process (no Dock icon while hidden), quick to reopen, explicit Quit only.

## 1. Tray setup (`src/tray.rs`)

Use [`tray-icon`](https://github.com/tauri-apps/tray-icon) (0.26) + `tray_icon::menu` (bundled menu API):

```rust
pub struct Tray {                    // leaks on purpose — NEVER dropped (dropping removes the icon)
    _icon: tray_icon::TrayIcon,
    _menu: tray_icon::menu::Menu,
}

pub fn init(cx: &mut App) -> Tray {
    // must run on the main thread, before/while the runloop is alive
    let menu = Menu::new();
    let open = MenuItem::with_id(MENU_OPEN, "Open Dislexia", true, None);
    let new_chat = MenuItem::with_id(MENU_NEW_CHAT, "New Chat", true, None);
    let settings = MenuItem::with_id(MENU_SETTINGS, "Settings…", true, None);
    let quit = MenuItem::with_id(MENU_QUIT, "Quit Dislexia", true, None);
    /* add separators, append items… */
    let icon = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(load_template_icon())     // [[2026-10-26-07-theme]] §5: template image, generated from media/dislexia.svg ([[2026-10-26-02-project-setup]] §8)
        .with_tooltip("Dislexia")
        .build().expect("tray icon");
    spawn_event_thread();                    // §3
    Tray { _icon: icon, _menu: menu }
}
```

macOS specifics: the `TrayIcon` must be created on the main thread (we're inside `Application::run`, fine) and kept alive for the process lifetime; keep it in the `Dislexia` root entity.

Left-click behavior: `TrayIconEvent::Click` → open main window (same as the menu item). This is the primary "open the app" gesture.

## 2. Menu

| Item | Action |
|---|---|
| **Open Dislexia** | Show/focus main window (§4) |
| **New Chat** | Show window (if hidden) + `NewChat` action |
| **Settings…** | Show window (if hidden) + open the settings dialog — system prompt, model, server, options |
| *(separator)* | |
| **Quit Dislexia** | `cx.quit()` |

Keep the menu minimal; settings etc. live in the window.

## 3. Event plumbing — tray → GPUI main runloop

`tray-icon` delivers events through global channels (`TrayIconEvent::receiver()`, `MenuEvent::receiver()` — crossbeam receivers). A dedicated std thread blocks on both and forwards into the app:

```rust
fn spawn_event_thread() {
    std::thread::spawn(|| {
        let menu_rx = MenuEvent::receiver();
        let tray_rx = TrayIconEvent::receiver();
        loop {
            crossbeam_channel::select! {           // or poll with timeout on both
                recv(menu_rx) -> ev => forward(ev),
                recv(tray_rx) -> ev => forward(ev),
            }
        }
    });
}
```

`forward` must land on the main thread. Preferred mechanism: capture a `WeakEntity<Dislexia>` at startup; in the forwarding closure run `entity.update(cx, |app, cx| app.handle_tray(ev, cx))` via the GPUI main-thread dispatch the pinned version offers (e.g. spawning onto the app's foreground executor / `App::spawn`). If entity APIs require window context, use the stored root entity handle, which lives app-wide. **Verify the exact main-thread hop API at M6** — this is the one integration in the app without a stock GPUI equivalent. Fallback that always works: `dispatch_get_main_queue()` closure via `objc2-foundation` that calls a function-pointer-based trampoline into a global holding the weak handle.

> Implementation note (M6): the hop is a GPUI foreground task (`App::spawn`)
> that drains both global receivers every 100ms — all entity access stays on
> the main thread by construction and no extra hop crate is needed; a 100ms
> poll is invisible at menu-click cadence. Risk #1 is resolved in the
> pinned GPUI: quitting when the last window closes is **opt-in** via
> `cx.on_window_closed` (see the `on_window_close_quit` example in the gpui
> sources), so the default keeps the process alive and the fallbacks were
> not needed. The root view entity is owned by its window, so the tray
> routes through an `AppRoot` global (weak root + window handle); when the
> window is gone, "Open"/"New Chat" recreate the window, which reloads
> settings and chats from disk. The `Tray` handle leaks by design — dropping
> a tray icon removes it — instead of living on the (droppable) root entity.

## 4. Window show / hide & Dock icon policy

State: an `AppRoot` global holds a weak `Entity<Dislexia>` + the `AnyWindowHandle` (the root view
entity is owned by its window, so a plain field would die with it — see the M6 note in §3). GPUI
destroys closed windows, so "reopen" means recreating the window and reloading state from disk.

**Show (tray open / reopen):**

```rust
fn show_main_window(cx: &mut App) {
    match root.upgrade() {
        Some(root) if window.is_alive() => { /* activate the existing window */ }
        _ => { /* open a fresh window around a new Dislexia built from settings + chats */ }
    }
    cx.activate(true);                                    // bring app forward
    set_dock_policy(DockPolicy::Regular);                 // §5
    platform::macos::activate_app();                      // accessory apps need the NSApp kick
}
```

**Close (`cmd-w`, red traffic light):** GPUI removes the window (no interception attempt in v1).
On last-window-closed we flip the Dock policy:

```rust
if cx.windows().is_empty() { set_dock_policy(DockPolicy::Accessory); }
```

> ⚠ **Risk #1 — auto-quit on last window close.** Verify at M6 whether the pinned GPUI quits the process when the last window closes (Zed itself quits by explicit logic, so the framework likely does **not**, but confirm). If it does, counter-measures in order of preference: (a) find the framework hook for window-close/quit-intent and veto it (hide instead), (b) keep an offscreen 1×1 hidden window alive so the app never becomes windowless, (c) intercept `cmd-q`/quit action and route everything except the tray menu through "hide". Add a manual test to [[2026-10-26-09-testing]] §3: close window, wait 60s, confirm the process and tray icon are alive.

## 5. Dock activation policy (`src/platform/macos.rs`)

```rust
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

pub enum DockPolicy { Regular, Accessory }

pub fn set_dock_policy(policy: DockPolicy) {
    let app = unsafe { NSApplication::sharedApplication() };
    let policy = match policy {
        DockPolicy::Regular  => NSApplicationActivationPolicy::Regular,
        DockPolicy::Accessory => NSApplicationActivationPolicy::Accessory,  // hides Dock icon
    };
    unsafe { app.setActivationPolicy(policy) };   // main thread only — we are
}

pub fn activate_app() {
    // NSRunningApplication::currentApplication().activateWithOptions(.activateIgnoringOtherApps)
    // required for Accessory apps to come to the front on reopen
}

/// Hides the native traffic lights once per window (`plan 03 §6`); returns
/// whether a live `NSWindow` was found.
pub fn hide_window_buttons(window: &gpui_kit::Window) -> bool;
```

Policy transitions: launch → Regular (window opens immediately at first run). While any window exists → Regular. Windowless → Accessory. Info.plist must **not** contain `LSUIElement` (decision #4 in [[2026-10-26-build-plan]]).

Note: switching Accessory→Regular can momentarily re-order window levels; call `activate_app()` **after** the policy flip, and file any flicker under M7 polish.

## 6. Lifecycle summary

| Event | Effect |
|---|---|
| Launch (login/manual) | Load settings+history → tray created → window opens → Dock: Regular |
| Close window (`cmd-w` / red button) | Window destroyed → Dock: Accessory → app + tray stay alive; generation continues streaming in background |
| Tray click / "Open" | Window recreated-or-focused, app activated, Dock: Regular |
| "New Chat" (tray) | Show window + new chat + focus input |
| `cmd-q` (window focused) or tray "Quit" | `cx.quit()` — only exit paths |
| macOS logout/shutdown | Standard termination; state already persisted per message ([[2026-10-26-06-chat-history]] §3) |

In-flight generations during background time keep streaming into `ChatStore` (chunks arrive on the background executor regardless of windows — entities update fine without a live window; the transcript simply re-renders on next open).

## 7. Background behavior contract (acceptance for M6)

1. After window close: process alive (`ps`), tray icon present, **no Dock icon**, no Stage Manager/mission-control presence.
2. A generation started before close completes and is persisted.
3. Reopen from tray restores the window with full state (chats, settings) — within < 500ms for typical history.
4. Quit works from both paths and never leaves the tray icon orphaned.

## 8. Stretch

- **Launch at login** via `SMAppService` (`objc2-app-kit` / `ServiceManagement`): toggle lives in Settings → General ([[2026-10-26-05-settings]] §1). Requires a bundled, signed app — schedule for after bundling (M7) and gate the switch to bundled builds only.
- Unread-message badge on the tray icon when a background generation completes.
- Global hotkey to open the window (e.g. `⌥Space`) — likely `keyboard shortcuts` via Carbon API or a crate like `global-hotkey` (same maintainers as `tray-icon`).

## External references

- [Stack Overflow: adding tray-icon to a GPUI app](https://stackoverflow.com/questions/79950357/how-can-i-add-a-tray-icon-to-a-rust-gui-application-with-gpui)
- [CrossPaste: creating the perfect macOS system tray (GPUI app)](https://crosspaste.com/en/blog/mac-system-tray)
- [tray-icon crate](https://github.com/tauri-apps/tray-icon)

---

## Follow-ups

- Risk #1 (auto-quit on last window close) is resolved for the pinned GPUI — see the M6 note in §3; the 60s-alive manual check is still unticked in [[2026-10-26-09-testing]] §3.
- Stretch: launch at login, unread badge on the tray icon, global hotkey (§8).
- Consistency check 2026-10-26: §1/§4/§5 now show the shipped menu API (`MenuItem::with_id`) and the `AppRoot`-global window routing, and the `platform::macos` signatures (`set_dock_policy`, `activate_app`, `hide_window_buttons`).

## References

- Related: [[2026-10-26-02-project-setup]], [[2026-10-26-07-theme]],
  [[2026-10-26-09-testing]], [[2026-10-26-10-milestones]], [[2026-10-26-build-plan]],
  [[2026-10-26-06-chat-history]], [[2026-10-26-05-settings]]
- Code: `src/tray.rs`, `src/platform/macos.rs`, `src/app.rs`
