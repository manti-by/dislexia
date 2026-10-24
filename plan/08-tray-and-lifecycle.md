# 08 — System tray & app lifecycle

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

Goal: Dislexia behaves like a menu-bar utility — **always in the tray**, survives window close as a background process (no Dock icon while hidden), quick to reopen, explicit Quit only.

## 1. Tray setup (`src/tray.rs`)

Use [`tray-icon`](https://github.com/tauri-apps/tray-icon) (0.26) + `tray_icon::menu` (bundled menu API):

```rust
pub struct Tray {                    // stored on Dislexia — NEVER dropped (dropping removes the icon)
    _icon: tray_icon::TrayIcon,
    _menu: tray_icon::menu::Menu,
}

pub fn init(cx: &mut App) -> Tray {
    // must run on the main thread, before/while the runloop is alive
    let menu = Menu::new();
    let open   = MenuItem::id_or_new("open");    // "Open Dislexia"
    let newc   = MenuItem::id_or_new("new_chat");// "New Chat"
    let quit   = MenuItem::id_or_new("quit");    // "Quit Dislexia"
    /* add separators, append items… */
    let icon = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(load_template_icon())     // 07 §5: template image, generated from media/icon-typewriter.svg (02 §8)
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

State: `main_window: Option<WindowHandle>` on `Dislexia` (recreated after close — GPUI destroys closed windows).

**Show (tray open / reopen):**

```rust
fn show_main_window(cx: &mut App) {
    match &self.main_window {
        Some(h) if h.is_alive() => { h.update(cx, |_, window, cx| window.activate_window()).ok(); }
        _ => { self.main_window = Some(cx.open_window(opts, build_workspace)); }
    }
    cx.activate(true);                                  // bring app forward
    set_dock_policy(DockPolicy::Regular, cx);           // §5
    platform::macos::activate_app_ignoring_other_apps(); // accessory apps need the NSApp kick
}
```

**Close (`cmd-w`, red traffic light, tray "Hide"):** GPUI removes the window (no interception attempt in v1). On last-window-closed we flip the Dock policy:

```rust
if cx.windows().is_empty() { set_dock_policy(DockPolicy::Accessory, cx); }
```

> ⚠ **Risk #1 — auto-quit on last window close.** Verify at M6 whether the pinned GPUI quits the process when the last window closes (Zed itself quits by explicit logic, so the framework likely does **not**, but confirm). If it does, counter-measures in order of preference: (a) find the framework hook for window-close/quit-intent and veto it (hide instead), (b) keep an offscreen 1×1 hidden window alive so the app never becomes windowless, (c) intercept `cmd-q`/quit action and route everything except the tray menu through "hide". Add a manual test to [09](09-testing.md) §3: close window, wait 60s, confirm the process and tray icon are alive.

## 5. Dock activation policy (`src/platform/macos.rs`)

```rust
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

pub enum DockPolicy { Regular, Accessory }

pub fn set_dock_policy(p: DockPolicy, _cx: &mut App) {
    let app = unsafe { NSApplication::sharedApplication() };
    let policy = match p {
        DockPolicy::Regular  => NSApplicationActivationPolicy::Regular,
        DockPolicy::Accessory => NSApplicationActivationPolicy::Accessory,  // hides Dock icon
    };
    unsafe { app.setActivationPolicy(policy) };   // main thread only — we are
}

pub fn activate_app_ignoring_other_apps() {
    // NSRunningApplication::currentApplication().activateWithOptions(.activateIgnoringOtherApps)
    // required for Accessory apps to come to the front on reopen
}
```

Policy transitions: launch → Regular (window opens immediately at first run). While any window exists → Regular. Windowless → Accessory. Info.plist must **not** contain `LSUIElement` (decision #4 in [BUILD_PLAN.md](../BUILD_PLAN.md)).

Note: switching Accessory→Regular can momentarily re-order window levels; call `activate_app_ignoring_other_apps()` **after** the policy flip, and file any flicker under M7 polish.

## 6. Lifecycle summary

| Event | Effect |
|---|---|
| Launch (login/manual) | Load settings+history → tray created → window opens → Dock: Regular |
| Close window (`cmd-w` / red button) | Window destroyed → Dock: Accessory → app + tray stay alive; generation continues streaming in background |
| Tray click / "Open" | Window recreated-or-focused, app activated, Dock: Regular |
| "New Chat" (tray) | Show window + new chat + focus input |
| `cmd-q` (window focused) or tray "Quit" | `cx.quit()` — only exit paths |
| macOS logout/shutdown | Standard termination; state already persisted per message ([06](06-chat-history.md) §3) |

In-flight generations during background time keep streaming into `ChatStore` (chunks arrive on the background executor regardless of windows — entities update fine without a live window; the transcript simply re-renders on next open).

## 7. Background behavior contract (acceptance for M6)

1. After window close: process alive (`ps`), tray icon present, **no Dock icon**, no Stage Manager/mission-control presence.
2. A generation started before close completes and is persisted.
3. Reopen from tray restores the window with full state (chats, settings) — within < 500ms for typical history.
4. Quit works from both paths and never leaves the tray icon orphaned.

## 8. Stretch

- **Launch at login** via `SMAppService` (`objc2-app-kit` / `ServiceManagement`): toggle lives in Settings → General (05 §1). Requires a bundled, signed app — schedule for after bundling (M7) and gate the switch to bundled builds only.
- Unread-message badge on the tray icon when a background generation completes.
- Global hotkey to open the window (e.g. `⌥Space`) — likely `keyboard shortcuts` via Carbon API or a crate like `global-hotkey` (same maintainers as `tray-icon`).

## References

- [Stack Overflow: adding tray-icon to a GPUI app](https://stackoverflow.com/questions/79950357/how-can-i-add-a-tray-icon-to-a-rust-gui-application-with-gpui)
- [CrossPaste: creating the perfect macOS system tray (GPUI app)](https://crosspaste.com/en/blog/mac-system-tray)
- [tray-icon crate](https://github.com/tauri-apps/tray-icon)
