//! The Ollama API client: pure blocking Rust over `ureq` 3 — **no GPUI
//! imports** (`wiki/pages/2026-10-26-04-ollama-client.md`). Runs unchanged on GPUI's
//! background executor and integration-tests against a mock server.

mod client;
mod types;

pub use client::{CancelFlag, OllamaClient, build_chat_request, user_error_message};
pub use types::{ApiMessage, ChatChunk, ChatRequest, DoneStats, OllamaError, Options};
