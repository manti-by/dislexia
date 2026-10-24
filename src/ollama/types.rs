//! Serde types for the Ollama REST API and the error taxonomy
//! (`plan/04-ollama-client.md` §3–5, §7).

use serde::{Deserialize, Serialize};

/// Errors surfaced by the client. The user-facing wording for each variant
/// lives in one place: `user_error_message` in `client.rs` (`plan/04 §7`).
#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    /// TCP refused/failed, DNS, unreachable host.
    #[error("connection failed: {0}")]
    Connection(String),
    /// Connect/read timeout.
    #[error("timed out")]
    Timeout,
    /// Non-2xx response.
    #[error("http {status}")]
    Http { status: u16, body: String },
    /// An `"error"` field from Ollama itself (status can still be 200).
    #[error("{0}")]
    Api(String),
    /// Line read error (socket dropped mid-stream and similar).
    #[error("io error: {0}")]
    Io(String),
    /// Malformed NDJSON.
    #[error("parse error: {0}")]
    Parse(String),
    /// The user pressed Stop; the loop exits with this before the next line.
    #[error("cancelled")]
    Cancelled,
}

impl OllamaError {
    pub fn is_cancelled(&self) -> bool {
        matches!(self, OllamaError::Cancelled)
    }
}

/// One message in a `/api/chat` request or response (`plan/04 §3`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
}

/// Generation options; `None` fields are omitted from the payload.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Options {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
}

impl Options {
    /// `true` when every field is `None` — the `options` object is omitted
    /// from the request entirely (`plan/04 §3`).
    pub fn is_empty(&self) -> bool {
        *self == Options::default()
    }
}

/// `POST /api/chat` payload. `stream` is always `true` — the only generation
/// path in v1 (`plan/BUILD_PLAN.md` decision 5).
#[derive(Clone, Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ApiMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Options>,
}

/// One NDJSON line of the stream. Ollama sends delta lines with
/// `done: false` and a final line with `done: true` carrying stats; the
/// final line repeats `message` with empty content, so a flat struct with
/// `done` as the discriminator covers both shapes (see `plan/04 §4`).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ChatChunk {
    #[serde(default)]
    pub message: DeltaMessage,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(flatten)]
    pub stats: DoneStats,
}

impl ChatChunk {
    /// The delta text of this line (empty on the done line).
    pub fn text(&self) -> &str {
        &self.message.content
    }

    /// Stats of the final line, if present.
    pub fn done_stats(&self) -> Option<&DoneStats> {
        self.done.then_some(&self.stats)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DeltaMessage {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: String,
}

/// Token statistics from the final stream line; all optional.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct DoneStats {
    #[serde(default)]
    pub total_duration: Option<u64>,
    #[serde(default)]
    pub eval_count: Option<u32>,
    #[serde(default)]
    pub eval_duration: Option<u64>,
    #[serde(default)]
    pub prompt_eval_count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELTA_LINE: &str = r#"{"model":"llama3.2","created_at":"2026-10-23T10:00:00Z","message":{"role":"assistant","content":"use "},"done":false}"#;
    const DONE_LINE: &str = r#"{"model":"llama3.2","created_at":"2026-10-23T10:00:02Z","message":{"role":"assistant","content":""},"done":true,"total_duration":1234567,"load_duration":0,"prompt_eval_count":26,"eval_count":298,"eval_duration":1100000000}"#;

    #[test]
    fn delta_line_parses() {
        let chunk: ChatChunk = serde_json::from_str(DELTA_LINE).unwrap();
        assert!(!chunk.done);
        assert_eq!(chunk.text(), "use ");
        assert_eq!(chunk.message.role, "assistant");
        assert!(chunk.done_stats().is_none());
    }

    #[test]
    fn done_line_parses_with_stats() {
        let chunk: ChatChunk = serde_json::from_str(DONE_LINE).unwrap();
        assert!(chunk.done);
        assert_eq!(chunk.text(), "");
        let stats = chunk.done_stats().unwrap();
        assert_eq!(stats.eval_count, Some(298));
        assert_eq!(stats.prompt_eval_count, Some(26));
        assert_eq!(stats.total_duration, Some(1_234_567));
    }

    #[test]
    fn done_line_without_stats_tolerates_missing_fields() {
        let line = r#"{"message":{"role":"assistant","content":""},"done":true}"#;
        let chunk: ChatChunk = serde_json::from_str(line).unwrap();
        assert!(chunk.done);
        assert_eq!(chunk.stats.eval_count, None);
    }

    #[test]
    fn mid_stream_error_surfaces() {
        let line = r#"{"error":"model requires more system memory"}"#;
        let chunk: ChatChunk = serde_json::from_str(line).unwrap();
        assert_eq!(
            chunk.error.as_deref(),
            Some("model requires more system memory")
        );
    }

    #[test]
    fn garbage_line_is_a_parse_error() {
        let result = serde_json::from_str::<ChatChunk>("not json at all {");
        assert!(result.is_err());
    }

    #[test]
    fn options_omit_none_fields_and_empty_object_is_dropped() {
        let options = Options {
            temperature: Some(0.7),
            top_k: Some(40),
            ..Options::default()
        };
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(json, r#"{"temperature":0.7,"top_k":40}"#);

        assert!(Options::default().is_empty());
        assert!(!options.is_empty());
    }

    #[test]
    fn request_round_trips_model_name_and_skips_empty_options() {
        let request = ChatRequest {
            model: "llama3.2:latest".to_owned(),
            messages: vec![ApiMessage {
                role: "user".to_owned(),
                content: "hi".to_owned(),
            }],
            stream: true,
            options: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""model":"llama3.2:latest""#));
        assert!(json.contains(r#""stream":true"#));
        assert!(!json.contains("options"));
    }
}
