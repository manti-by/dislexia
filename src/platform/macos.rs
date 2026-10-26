//! objc2 helpers: Dock activation policy, app activation and window chrome
//! (`wiki/pages/2026-10-26-08-tray-and-lifecycle.md` §5,
//! `wiki/pages/2026-10-26-03-ui-layout.md` §6). Only `main.rs`, `app.rs` and
//! `tray.rs` may use this.
//!
//! macOS-only by design; the module boundary keeps a port thinkable.

use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy,
    NSRunningApplication, NSView, NSWindowButton,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Where the app icon lives: in the Dock with windows open, or hidden while
/// the app sits in the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockPolicy {
    Regular,
    Accessory,
}

/// Sets the Dock activation policy (main thread only — callers are).
pub fn set_dock_policy(policy: DockPolicy) {
    let mtm = objc2::MainThreadMarker::new().expect("dock policy is set on the main thread");
    let ns_policy = match policy {
        DockPolicy::Regular => NSApplicationActivationPolicy::Regular,
        DockPolicy::Accessory => NSApplicationActivationPolicy::Accessory,
    };
    // setActivationPolicy is main-thread-only but not `unsafe` in objc2 0.6;
    // the MainThreadMarker is the proof we are allowed to call it.
    NSApplication::sharedApplication(mtm).setActivationPolicy(ns_policy);
}

/// Brings the app to the front on reopen: Accessory apps need the explicit
/// activation kick (`plan 08 §5`). `ActivateIgnoringOtherApps` is deprecated
/// on macOS 14+ (where plain activation suffices) but remains the API the
/// objc2 bindings expose.
pub fn activate_app() {
    #[allow(deprecated)]
    let options = NSApplicationActivationOptions::ActivateIgnoringOtherApps;
    NSRunningApplication::currentApplication().activateWithOptions(options);
}

/// Hides AppKit's standard close/minimize/zoom buttons so the app can draw
/// its own 12px controls (`plan 03 §6`). Returns whether a live native window
/// was reached; callers may retry on the next frame while it is false.
pub fn hide_window_buttons(window: &gpui_kit::Window) -> bool {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return false;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return false;
    };
    // SAFETY: GPUI hands out the `NSView` of a live window on the main thread,
    // which is where this is called from.
    let view = unsafe { &*(handle.ns_view.as_ptr() as *const NSView) };
    let Some(native_window) = view.window() else {
        return false;
    };
    for button in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = native_window.standardWindowButton(button) {
            button.setHidden(true);
        }
    }
    true
}
