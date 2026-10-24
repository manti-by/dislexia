//! The blocking Ollama client (`plan/04-ollama-client.md` §1, §5–7).

use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;

use crate::history::{Message, MessageStatus, Role};

use super::types::{ApiMessage, ChatChunk, ChatRequest, OllamaError, Options};

/// Connect timeout: "server down" is detected fast (`plan/04 §7`).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Read timeout between bytes; also bounds the worst-case Stop latency.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct OllamaClient {
    base: String,
    agent: ureq::Agent,
}

impl OllamaClient {
    /// Builds a client for `server_url` (already normalized by settings).
    pub fn new(server_url: &str) -> Result<Self, OllamaError> {
        let base = server_url.trim_end_matches('/').to_owned();
        if base.is_empty() {
            return Err(OllamaError::Connection("empty server URL".to_owned()));
        }
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(READ_TIMEOUT))
            // Non-2xx responses are inspected here (we need the error body),
            // not turned into exceptions by ureq.
            .http_status_as_error(false)
            .build()
            .new_agent();
        Ok(Self { base, agent })
    }

    /// `GET /api/tags` — installed model names (`plan/04 §2`).
    pub fn list_models(&self) -> Result<Vec<String>, OllamaError> {
        let response = self.agent.get(format!("{}/api/tags", self.base)).call();
        let response = map_transport_error(response)?;
        let body = read_body(response)?;
        #[derive(Deserialize)]
        struct Tags {
            #[serde(default)]
            models: Vec<ModelTag>,
        }
        #[derive(Deserialize)]
        struct ModelTag {
            name: String,
        }
        let tags: Tags =
            serde_json::from_str(&body).map_err(|error| OllamaError::Parse(error.to_string()))?;
        Ok(tags.models.into_iter().map(|model| model.name).collect())
    }

    /// Connection test = a successful `/api/tags` with its model count
    /// (`plan/04 §1`).
    pub fn test_connection(&self) -> Result<usize, OllamaError> {
        self.list_models().map(|models| models.len())
    }

    /// `POST /api/chat` with `stream: true`; returns a blocking iterator over
    /// the NDJSON chunks (`plan/04 §5`). The HTTP status/body errors surface
    /// immediately; per-line errors surface while iterating.
    pub fn chat(
        &self,
        request: &ChatRequest,
        cancel: &CancelFlag,
    ) -> Result<ChatStream, OllamaError> {
        let response = self
            .agent
            .post(format!("{}/api/chat", self.base))
            .send_json(request);
        let response = map_transport_error(response)?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(http_error(status, read_body(response)?));
        }
        Ok(ChatStream {
            lines: std::io::BufReader::new(response.into_body().into_reader()).lines(),
            cancel: cancel.clone(),
        })
    }
}

/// Blocking NDJSON line iterator with between-line cancellation
/// (`plan/04 §5–6`).
pub struct ChatStream {
    lines: std::io::Lines<std::io::BufReader<ureq::BodyReader<'static>>>,
    cancel: CancelFlag,
}

impl std::fmt::Debug for ChatStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChatStream")
            .field("cancelled", &self.cancel.is_cancelled())
            .finish()
    }
}

impl Iterator for ChatStream {
    type Item = Result<ChatChunk, OllamaError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(cancelled) = self.cancel.check() {
            return Some(Err(cancelled));
        }
        let line = match self.lines.next() {
            Some(Ok(line)) => line,
            Some(Err(error)) => return Some(Err(OllamaError::Io(error.to_string()))),
            None => return None,
        };
        if line.trim().is_empty() {
            return self.next();
        }
        let chunk: ChatChunk = match serde_json::from_str(&line) {
            Ok(chunk) => chunk,
            Err(error) => return Some(Err(OllamaError::Parse(error.to_string()))),
        };
        if let Some(message) = chunk.error.clone() {
            return Some(Err(OllamaError::Api(message)));
        }
        Some(Ok(chunk))
    }
}

/// Cancellation flag shared with the UI (`plan/04 §6`). The Stop button sets
/// it; the stream loop checks it between lines.
#[derive(Clone)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// `Err(OllamaError::Cancelled)` when the flag is set.
    pub fn check(&self) -> Result<(), OllamaError> {
        if self.is_cancelled() {
            Err(OllamaError::Cancelled)
        } else {
            Ok(())
        }
    }
}

impl Default for CancelFlag {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds the `/api/chat` payload for a chat turn (`plan/04 §3`, `plan/06 §6`):
/// system message first (only when non-empty), then every stored message
/// except errors and live-streaming placeholders.
pub fn build_chat_request(
    model: &str,
    system_prompt: &str,
    options: &Options,
    history: &[Message],
) -> ChatRequest {
    let mut messages = Vec::with_capacity(history.len() + 1);
    if !system_prompt.trim().is_empty() {
        messages.push(ApiMessage {
            role: "system".to_owned(),
            content: system_prompt.to_owned(),
        });
    }
    for message in history {
        let replay = matches!(
            message.status,
            MessageStatus::Complete | MessageStatus::Stopped
        );
        if !replay {
            continue;
        }
        messages.push(ApiMessage {
            role: match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            }
            .to_owned(),
            content: message.content.clone(),
        });
    }
    ChatRequest {
        model: model.to_owned(),
        messages,
        stream: true,
        options: (!options.is_empty()).then(|| options.clone()),
    }
}

/// The user-facing wording for every error variant, in one place
/// (`plan/04 §7`). Logged errors keep their debug form; this copy is what
/// lands in the transcript.
pub fn user_error_message(error: &OllamaError, server_url: &str, model: &str) -> String {
    match error {
        OllamaError::Connection(_) => {
            format!("Can't reach Ollama at {server_url} — is \"ollama serve\" running?")
        }
        OllamaError::Timeout => "Ollama timed out. Try again or check the model.".to_owned(),
        OllamaError::Http { status: 404, .. } => {
            format!("Model \"{model}\" not found. Open Settings → Model.")
        }
        OllamaError::Http { status, body } => {
            let first_line = body.lines().next().unwrap_or_default();
            format!("Ollama error {status}: {first_line}")
        }
        OllamaError::Api(message) => message.clone(),
        OllamaError::Parse(_) => "Got an unexpected response from Ollama (parse error).".to_owned(),
        OllamaError::Io(_) => "Connection to Ollama was interrupted.".to_owned(),
        OllamaError::Cancelled => "Stopped.".to_owned(),
    }
}

fn map_transport_error(
    result: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<ureq::http::Response<ureq::Body>, OllamaError> {
    match result {
        Ok(response) => Ok(response),
        Err(ureq::Error::StatusCode(status)) => Err(OllamaError::Http {
            status,
            body: String::new(),
        }),
        Err(ureq::Error::Timeout(_)) => Err(OllamaError::Timeout),
        Err(ureq::Error::Io(error)) => Err(transport_kind(&error.to_string(), error)),
        Err(ureq::Error::HostNotFound) => Err(OllamaError::Connection("host not found".to_owned())),
        Err(ureq::Error::ConnectionFailed) => {
            Err(OllamaError::Connection("connection failed".to_owned()))
        }
        Err(ureq::Error::BadUri(url)) => Err(OllamaError::Connection(format!("bad URL: {url}"))),
        Err(other) => Err(OllamaError::Connection(other.to_string())),
    }
}

/// ureq reports refused/dropped connections as `Io`; split out the ones the
/// user reads as "server down" (`plan/04 §7`).
fn transport_kind(message: &str, error: std::io::Error) -> OllamaError {
    if message.contains("refused")
        || message.contains("unreachable")
        || message.contains("Connection reset")
        || message.contains("Broken pipe")
        || message.contains("connect")
    {
        OllamaError::Connection(message.to_owned())
    } else {
        OllamaError::Io(error.to_string())
    }
}

fn http_error(status: u16, body: String) -> OllamaError {
    // Ollama's error bodies are `{"error": "..."}` — prefer that message.
    #[derive(Deserialize)]
    struct ErrorBody {
        error: Option<String>,
    }
    if let Ok(parsed) = serde_json::from_str::<ErrorBody>(&body)
        && let Some(message) = parsed.error
    {
        return OllamaError::Http {
            status,
            body: message,
        };
    }
    OllamaError::Http { status, body }
}

fn read_body(response: ureq::http::Response<ureq::Body>) -> Result<String, OllamaError> {
    response
        .into_body()
        .read_to_string()
        .map_err(|error| OllamaError::Io(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::MessageStatus;
    use chrono::Utc;

    fn message(role: Role, content: &str, status: MessageStatus) -> Message {
        Message {
            role,
            content: content.to_owned(),
            created_at: Utc::now(),
            status,
        }
    }

    #[test]
    fn system_prompt_prepended_only_when_non_empty() {
        let history = vec![message(Role::User, "hi", MessageStatus::Complete)];

        let with_prompt = build_chat_request("m", "fix text", &Options::default(), &history);
        assert_eq!(with_prompt.messages.len(), 2);
        assert_eq!(with_prompt.messages[0].role, "system");
        assert_eq!(with_prompt.messages[0].content, "fix text");

        let without = build_chat_request("m", "", &Options::default(), &history);
        assert_eq!(without.messages.len(), 1);
        assert_eq!(without.messages[0].role, "user");

        let blank = build_chat_request("m", "   ", &Options::default(), &history);
        assert_eq!(blank.messages.len(), 1);
    }

    #[test]
    fn error_and_streaming_messages_are_not_replayed() {
        let history = vec![
            message(Role::User, "first", MessageStatus::Complete),
            message(
                Role::Assistant,
                "broken",
                MessageStatus::Error {
                    message: "boom".into(),
                },
            ),
            message(Role::User, "second", MessageStatus::Complete),
            message(Role::Assistant, "partial", MessageStatus::Stopped),
            message(
                Role::Assistant,
                "live",
                MessageStatus::Streaming {
                    started_at: Utc::now(),
                    model: "m".into(),
                },
            ),
        ];
        let request = build_chat_request("m", "", &Options::default(), &history);
        let contents: Vec<&str> = request
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect();
        assert_eq!(contents, vec!["first", "second", "partial"]);
    }

    #[test]
    fn options_object_omitted_entirely_when_all_none() {
        let empty = build_chat_request("m", "", &Options::default(), &[]);
        assert!(empty.options.is_none());
        let json = serde_json::to_string(&empty).unwrap();
        assert!(!json.contains("options"));

        let options = Options {
            seed: Some(42),
            ..Options::default()
        };
        let with_options = build_chat_request("m", "", &options, &[]);
        let json = serde_json::to_string(&with_options).unwrap();
        assert!(json.contains(r#""options":{"seed":42}"#), "{json}");
    }

    #[test]
    fn error_wording_follows_the_mapping_table() {
        let url = "http://127.0.0.1:11434";
        assert_eq!(
            user_error_message(&OllamaError::Connection("refused".into()), url, "m"),
            "Can't reach Ollama at http://127.0.0.1:11434 — is \"ollama serve\" running?"
        );
        assert_eq!(
            user_error_message(&OllamaError::Timeout, url, "m"),
            "Ollama timed out. Try again or check the model."
        );
        assert_eq!(
            user_error_message(
                &OllamaError::Http {
                    status: 404,
                    body: "model 'x' not found".into()
                },
                url,
                "llama3.2"
            ),
            "Model \"llama3.2\" not found. Open Settings → Model."
        );
        assert_eq!(
            user_error_message(
                &OllamaError::Http {
                    status: 500,
                    body: "boom\nmore".into()
                },
                url,
                "m"
            ),
            "Ollama error 500: boom"
        );
        assert_eq!(
            user_error_message(&OllamaError::Api("memory".into()), url, "m"),
            "memory"
        );
        assert_eq!(
            user_error_message(&OllamaError::Parse("bad".into()), url, "m"),
            "Got an unexpected response from Ollama (parse error)."
        );
    }
}
