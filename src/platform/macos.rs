//! objc2 helpers: Dock activation policy and app activation
//! (`plan/08 §5`). Only `main.rs`, `app.rs` and `tray.rs` may use this.
//!
//! macOS-only by design; the module boundary keeps a port thinkable.

use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy,
    NSRunningApplication,
};

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
/// activation kick (`plan/08 §5`). `ActivateIgnoringOtherApps` is deprecated
/// on macOS 14+ (where plain activation suffices) but remains the API the
/// objc2 bindings expose.
pub fn activate_app() {
    #[allow(deprecated)]
    let options = NSApplicationActivationOptions::ActivateIgnoringOtherApps;
    NSRunningApplication::currentApplication().activateWithOptions(options);
}
