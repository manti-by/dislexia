# 04 — Ollama client

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

The `ollama` module is **pure blocking Rust** (no GPUI imports) so it is unit-testable and runs unchanged on GPUI's background executor. All HTTP goes through `ureq` 3.

## 1. Client

```rust
pub struct OllamaClient {
    base: String,          // from settings.server_url, no trailing slash
    agent: ureq::Agent,    // shared: connect timeout 3s, read timeout 30s
}

impl OllamaClient {
    pub fn new(server_url: &str) -> Result<Self, OllamaError>;   // validates + builds agent
    pub fn list_models(&self) -> Result<Vec<String>, OllamaError>;
    pub fn test_connection(&self) -> Result<usize, OllamaError>; // = list_models().len()
    /// Blocking iterator over stream chunks. See §5–6 for loop/cancellation contract.
    pub fn chat(&self, req: ChatRequest) -> Result<impl Iterator<Item = Result<ChatChunk, OllamaError>>, OllamaError>;
}
```

The client is rebuilt whenever `server_url` changes in settings.

## 2. Endpoints used

| Endpoint | Method | Purpose |
|---|---|---|
| `/api/tags` | GET | List installed models (settings dropdown, connection test) |
| `/api/chat` | POST, `stream: true` | Chat completion (the only generation path) |

Not used in v1: `/api/generate`, `/api/embeddings`, `/api/show`, `/api/pull`, `/api/delete`.

Default server URL: `http://127.0.0.1:11434`.

## 3. Request schema — `POST /api/chat`

```json
{
  "model": "llama3.2:latest",
  "messages": [
    { "role": "system",    "content": "<settings.system_prompt>" },
    { "role": "user",      "content": "how do I reverse a Vec in place?" },
    { "role": "assistant", "content": "…" },
    { "role": "user",      "content": "and for a slice?" }
  ],
  "stream": true,
  "options": {
    "temperature": 0.7,
    "top_p": 0.9,
    "top_k": 40,
    "num_predict": 128,
    "num_ctx": 4096,
    "repeat_penalty": 1.1,
    "seed": 42
  }
}
```

Rules:

- The **system message is always first** and only present when `system_prompt` is non-empty.
- The full conversation history is resent each turn (`/api/chat` is stateless). Messages stored in the chat file ([06](06-chat-history.md)) are exactly what gets replayed; error-status assistant messages are **not** replayed.
- `options` contains only fields the user set (`Option` fields serialize with `skip_serializing_if`); `options` omitted entirely when empty.
- Mapping to settings: see [05-settings.md](05-settings.md) §2.

## 4. Response — NDJSON stream

HTTP 200 with `application/x-ndjson`; each line is one JSON object. Line-by-line:

```json
{"model":"llama3.2","created_at":"2026-10-23T10:00:00Z","message":{"role":"assistant","content":"use "},"done":false}
{"model":"llama3.2","created_at":"2026-10-23T10:00:01Z","message":{"role":"assistant","content":"v.reverse();"},"done":false}
{"model":"llama3.2","created_at":"2026-10-23T10:00:02Z","message":{"role":"assistant","content":""},"done":true,
 "total_duration":1234567,"load_duration":0,"prompt_eval_count":26,"eval_count":298,"eval_duration":1100000000}
```

Rust types (`ollama/types.rs`):

```rust
#[derive(Serialize)] pub struct ChatRequest { model: String, messages: Vec<ApiMessage>,
                                              #[serde(default = "yes")] stream: bool,
                                              #[serde(skip_serializing_if="Option::is_none")] options: Option<Options> }
#[derive(Serialize, Deserialize)] pub struct ApiMessage { pub role: String, pub content: String }

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ChatChunk {
    Delta { #[serde(default)] message: DeltaMessage, done: bool },
    Done  { done: bool, /* stats fields, all optional: */ #[serde(flatten)] stats: DoneStats },
}

#[derive(Deserialize)] pub struct DeltaMessage { #[serde(default)] pub role: String, pub content: String }
#[derive(Deserialize, Default)]
pub struct DoneStats {
    pub total_duration: Option<u64>, pub eval_count: Option<u32>,
    pub eval_duration: Option<u64>,  pub prompt_eval_count: Option<u32>,
}
```

`#[serde(untagged)]` handles the two shapes; the done-line detection is `done == true`. If an `error` field is present (server-side errors can appear mid-stream), surface it as `OllamaError::Api`.

## 5. Streaming loop (background executor)

```rust
let resp = agent.post(url).send_json(&req)?;          // non-2xx → OllamaError::Http
let reader = BufReader::new(resp.into_reader());
for line in reader.lines() {
    let line = line?;
    cancelled.check()?;                               // §6 — between lines
    if line.trim().is_empty() { continue; }
    let chunk: ChatChunk = serde_json::from_str(&line)?;
    if let Some(err) = chunk.error() { return Err(OllamaError::Api(err)); }
    yield chunk;                                      // forwarded to ChatStore via WeakEntity update
}
```

Each yielded delta appends to the streaming assistant message and calls `cx.notify()` exactly once per line. Note GPUI batches notifications per frame, so per-line notify is safe.

## 6. Cancellation

- `GenerationHandle { flag: Arc<AtomicBool>, chat_id, started_at }` stored on `Dislexia`.
- Stop button / `esc` sets `flag = true`; the loop's `cancelled.check()` returns `Err(Cancelled)` before parsing the next line.
- Worst-case latency = one read timeout (30s) if the server stalls mid-line; acceptable for v1. (Do not try to hard-close the socket from another thread with `ureq`; document this as a known limitation.)
- On cancel: keep the partial assistant text, status = `Complete` with meta "(stopped)".

## 7. Error taxonomy → UI copy

| `OllamaError` variant | Trigger | UI message |
|---|---|---|
| `Connection` | TCP refused/failed | `Can't reach Ollama at {url} — is "ollama serve" running?` |
| `Timeout` | connect/read timeout | `Ollama timed out. Try again or check the model.` |
| `Http { 404 }` | unknown model | `Model "{model}" not found. Open Settings → Model.` |
| `Http { status, body }` | other non-200 | `Ollama error {status}: {first line of body}` |
| `Api { message }` | `"error"` field in stream | raw message |
| `Parse` | malformed NDJSON | `Got an unexpected response from Ollama (parse error).` |
| `Io` | line read error | generic wording + log |

Also: 3s connect timeout means "server down" is detected fast and does not block the UI (which stays responsive regardless — the call is on the background executor).

## 8. Alternative considered (not chosen)

`gpui-kit` re-exports an `http_client` module (async HTTP abstraction from the Zed stack). Rejected for v1: its async surface pulls in the executor coupling and gains us nothing over a blocking `BufRead` loop for NDJSON. Revisit only if we need request middleware (auth, proxies).
