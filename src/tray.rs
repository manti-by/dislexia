//! System tray: icon (template PNG generated from the icon SVG), minimal
//! menu, and event forwarding into the GPUI main runloop
//! (`wiki/pages/2026-10-26-08-tray-and-lifecycle.md`).
//!
//! The `Tray` struct must be kept alive for the whole process — dropping it
//! removes the icon.

use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::app;

pub const MENU_OPEN: &str = "dislexia.open";
pub const MENU_NEW_CHAT: &str = "dislexia.new-chat";
pub const MENU_SETTINGS: &str = "dislexia.settings";
pub const MENU_QUIT: &str = "dislexia.quit";

/// The tray handle; stored on the app for the process lifetime.
pub struct Tray {
    _icon: TrayIcon,
    _menu: Menu,
}

/// Creates the tray icon + menu on the main thread and starts the event
/// forwarder (`plan 08 §1, §3`). The `Tray` handle leaks on purpose: it must
/// outlive every window and dropping it would remove the icon.
pub fn init(cx: &mut gpui_kit::App) {
    let menu = Menu::new();
    let open = MenuItem::with_id(MENU_OPEN, "Open Dislexia", true, None);
    let new_chat = MenuItem::with_id(MENU_NEW_CHAT, "New Chat", true, None);
    let settings = MenuItem::with_id(MENU_SETTINGS, "Settings…", true, None);
    let quit = MenuItem::with_id(MENU_QUIT, "Quit Dislexia", true, None);
    menu.append_items(&[
        &open,
        &new_chat,
        &settings,
        &PredefinedMenuItem::separator(),
        &quit,
    ])
    .expect("tray menu builds");

    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        // Template rendering adapts to light/dark menu bars (`plan 07 §5`).
        .with_icon_templated(load_icon())
        .with_tooltip("Dislexia")
        // Left click opens the window (plan 08 §1); the menu lives on the
        // right click.
        .with_menu_on_left_click(false)
        .build()
        .expect("tray icon builds");

    spawn_event_forwarder(cx);
    std::mem::forget(Tray {
        _icon: icon,
        _menu: menu,
    });
}

/// Loads the 16px template PNG compiled into the binary (`plan 02 §8`).
fn load_icon() -> tray_icon::Icon {
    let image = image::load_from_memory(include_bytes!("../assets/tray-16@1x.png"))
        .expect("tray PNG is bundled");
    let rgba = image.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height).expect("tray icon is valid RGBA")
}

/// One tray intent, resolved on the main runloop.
enum Intent {
    Open,
    NewChat,
    OpenSettings,
    Quit,
}

/// Forwards tray + menu events onto the main thread (`plan 08 §3`).
///
/// Implementation note (M6): the plan sketched a dedicated std thread with
/// a main-thread hop. The pinned GPUI offers a simpler always-correct hop —
/// a foreground task polling both global event receivers — which keeps all
/// entity access on the main thread by construction. A 100ms poll is
/// invisible at menu-click cadence.
fn spawn_event_forwarder(cx: &mut gpui_kit::App) {
    cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
        loop {
            for intent in drain_intents() {
                match intent {
                    Intent::Open => {
                        cx.update(app::show_main_window);
                    }
                    Intent::NewChat => {
                        cx.update(app::new_chat_from_tray);
                    }
                    Intent::OpenSettings => {
                        cx.update(app::open_settings_from_tray);
                    }
                    Intent::Quit => {
                        log::info!("quit requested from the tray");
                        cx.update(|cx| cx.quit());
                    }
                }
            }
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
        }
    })
    .detach();
}

fn drain_intents() -> Vec<Intent> {
    let mut intents = Vec::new();
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        let id = event.id().as_ref();
        intents.push(match id {
            MENU_OPEN => Intent::Open,
            MENU_NEW_CHAT => Intent::NewChat,
            MENU_SETTINGS => Intent::OpenSettings,
            MENU_QUIT => Intent::Quit,
            other => {
                log::debug!("unhandled tray menu id: {other}");
                continue;
            }
        });
    }
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        // Any left-button click on the icon opens the window (`plan 08 §1`).
        if let TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        } = event
        {
            intents.push(Intent::Open);
        }
    }
    intents
}
