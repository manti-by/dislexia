//! Dislexia: a macOS menu-bar chat client for local Ollama models.
//!
//! The binary wiring lives in `src/main.rs`; everything testable is here.
//! The implementation contract is `BUILD_PLAN.md` and `plan/` — read the
//! matching `plan/0N-*.md` before changing a module.

pub mod app;
pub mod ollama;
pub mod platform;
pub mod settings;
pub mod theme;
pub mod tray;
pub mod ui;
