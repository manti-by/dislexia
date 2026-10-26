//! Dislexia: a macOS menu-bar chat client for local Ollama models.
//!
//! The binary wiring lives in `src/main.rs`; everything testable is here.
//! The implementation contract is the build plan in the wiki — start at
//! `wiki/pages/2026-10-26-build-plan.md` and read the matching
//! `wiki/pages/2026-10-26-0N-*.md` before changing a module.

pub mod app;
pub mod history;
pub mod ollama;
pub mod platform;
pub mod settings;
pub mod theme;
pub mod tray;
pub mod ui;
