# 05 — Settings

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

## 1. Schema (`src/settings.rs`)

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "snake_case", deny_unknown_fields)]
pub struct Settings {
    pub server_url: String,        // "http://127.0.0.1:11434"
    pub model: Option<String>,     // None until user (or first-connect autodetect) picks one
    pub system_prompt: String,     // default = DEFAULT_SYSTEM_PROMPT (§1.1); "" = no system message sent
    pub options: ModelOptions,
    pub launch_at_login: bool,     // stretch — UI switch disabled until 08 §8 lands
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, skip_serializing_if = "Option::is_none", deny_unknown_fields)]
pub struct ModelOptions {
    pub temperature: Option<f64>,     // 0.0–2.0
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub num_predict: Option<i32>,
    pub num_ctx: Option<u32>,
    pub repeat_penalty: Option<f64>,
    pub seed: Option<i64>,
}
```

`serde(default)` on both structs = forward/backward compatible file format (new fields get defaults, unknown fields are ignored rather than erroring — `deny_unknown_fields` is intentionally **omitted** at file level; keep it off).

Defaults (`impl Default for Settings`): `server_url = "http://127.0.0.1:11434"`, `system_prompt = DEFAULT_SYSTEM_PROMPT` (§1.1), `temperature = Some(0.2)` — low on purpose: the default persona is deterministic text editing, not creative writing; users can raise it in Settings. Others `None`, `launch_at_login = false`.

## 1.1 Default system prompt — the text fixer

The product default turns the model into an editor: fix spelling and wording, replace old-fashioned words, simplify the text while keeping it formal. The canonical copy lives in [`prompts/default-system-prompt.md`](../prompts/default-system-prompt.md) and is embedded into the binary at compile time:

```rust
// src/settings.rs
pub const DEFAULT_SYSTEM_PROMPT: &str = include_str!("../../prompts/default-system-prompt.md");
```

Rules:

- The default applies only when no `settings.json` exists (or the field was never written). Once the user edits the prompt, **their value wins** — upgrades never overwrite a saved prompt, even if the default changes later.
- Empty string still means "send no system message" ([04](04-ollama-client.md) §3).
- The Settings UI offers **Reset to default** (03 §2), which writes `DEFAULT_SYSTEM_PROMPT` back and saves.
- Treat the prompt file as product copy: change it deliberately, keep it short enough for small local models.

## 2. Field ↔ Ollama mapping

| Settings field | `/api/chat` payload |
|---|---|
| `server_url` | request URL base (`{server_url}/api/chat`, `/api/tags`) |
| `model` | `model` (send blocked while `None` — UI hint, see 03 §3) |
| `system_prompt` | first `messages` element, only when non-empty |
| `options.*` | `options` object fields, only when `Some` |

## 3. Validation & normalization (on save)

- `server_url`: trim, must parse as `http(s)://…`; strip trailing `/`; if no scheme, prefix `http://`. Invalid → keep previous value, show inline error in the settings field.
- Numeric clamps: `temperature ∈ [0, 2]`, `top_p ∈ [0,1]`, `top_k ≥ 0`, `repeat_penalty ∈ [0,3]`. Out-of-range → field error, not silent clamp.
- `model` must be one of the last `/api/tags` response when possible; if the stored model disappears, keep it but mark in the dropdown "(missing on server)".

## 4. Storage

- Path: `~/Library/Application Support/dislexia/settings.json` (created at startup).
- **Atomic writes**: serialize → write `settings.json.tmp` → `fs::rename` over the target. Never a partially-written file.
- Load once at startup (missing file → defaults, no error surfaced).
- Save on **every committed change** (field blur / debounced 300ms for text fields), performed on the background executor. Settings is an entity; `cx.notify()` after load and after each save.

Example file:

```json
{
  "server_url": "http://127.0.0.1:11434",
  "model": "llama3.2:latest",
  "system_prompt": "You are a writing editor. The user sends you text and you return an improved version of it. …(full default: prompts/default-system-prompt.md)",
  "options": { "temperature": 0.7, "top_k": 40 }
}
```

## 5. Change propagation

| Change | Effect |
|---|---|
| `server_url` | rebuild `OllamaClient`; re-run connection test; refresh model list |
| `model` | next send uses it; title bar label updates |
| `system_prompt`, `options.*` | affect the next request only (no retroactive change to stored chats) |

No "apply" button anywhere — instant effect, matching 03 §2.

## 6. First-run experience

1. Settings load defaults (no file).
2. Background: `test_connection()`.
   - **OK** → fetch `/api/tags`; if `model == None` and models exist → set `model` to the first, show toast "Using llama3.2:latest — change it in Settings".
   - **Fail** → status dot red in sidebar; transcript shows a connect-hint empty state (03 §3); retry happens when the user opens Settings → Test or sends a message.
