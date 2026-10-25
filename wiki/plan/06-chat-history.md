# 06 — Chat history

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

## 1. Data model (`src/history.rs`)

```rust
pub struct Chat {
    pub id: Uuid,                    // uuid v4, generated on creation
    pub title: String,               // auto from first user message (§4), user-renamable (stretch)
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,   // bumped on every appended message / completed stream
    pub messages: Vec<Message>,
}

pub struct Message {
    pub role: Role,                  // User | Assistant
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub status: MessageStatus,
}

pub enum MessageStatus {
    Complete,
    Streaming { started_at: DateTime<Utc>, model: String },
    Stopped,                         // cancelled by user, partial text kept
    Error(String),                   // error text; NOT replayed to the API
}
```

On-disk serialization uses the same shapes (`serde`; `MessageStatus` serializes as tagged enum). `ChatStore` (an entity) holds `chats: Vec<Chat>` fully in memory — hundreds of chats × a few hundred KB is fine; no lazy loading in v1.

## 2. Storage format

- One file per chat: `~/Library/Application Support/dislexia/chats/<uuid>.json`
- Atomic write (tmp + rename), background executor, fire-and-forget; errors are logged, not modal'd.
- `updated_at` is the sort key everywhere.
- Orphan/corrupt files at load: log a warning and skip the file (do not crash; do not delete user data). Keep a `chats/<uuid>.json.bad` rename for unparseable files so nothing is silently lost.

## 3. Lifecycle

| Event | Behavior |
|---|---|
| App launch | Load all chat files; if none, start with a fresh untitled chat (in-memory only) |
| New chat (`cmd-n`) | Fresh in-memory chat; **file is created lazily on first send** (no files full of empty chats) |
| First user message | Set `title = first_user_message` (see §4) |
| Every message append / chunk? | Persist **after** each user message, and **after** a stream finishes/stops/errors (not per chunk) |
| Delete chat | Confirm popover → remove from memory + `fs::remove_file` (background) |
| Window close / quit | No-op needed — everything already persisted at message granularity |

## 4. Titles

- From first user message: take first line, collapse whitespace, truncate to 48 chars + `…`, strip markdown fences.
- Sidebar grouping (03 §1): **Today** / **Yesterday** / previous 7 days / **Older**, each newest-first, by `updated_at` (local time).
- Rename via double-click or context menu: stretch (M7), title field is already just a string.

## 5. Sidebar listing

`ChatStore` exposes a derived, sorted `Vec<&Chat>` (by `updated_at` desc) the sidebar renders through the kit list; active chat highlighted with accent bar. Selection state (`active_chat_id`) lives on `ChatStore`.

## 6. API replay rules (with [04](04-ollama-client.md) §3)

When building a request for chat `X`:

```
messages = [ system? ]                      // from settings, if non-empty
         ++ X.messages.iter()
              .filter(|m| matches!(m.status, Complete | Stopped))   // skip Error, skip live Streaming
              .map(|m| ApiMessage { role, content })
         ++ [ just-typed user message ]     // already appended to X before the call
```

Multi-turn context therefore "just works" — each turn resends the whole (cleaned) history. If `messages` grows beyond a rough budget (~80k chars), v1 does **nothing** special (Ollama truncates by `num_ctx`); a "context too long" indicator is a possible M7 addition.

## 7. Stretch (post-v1)

- Export chat to Markdown file
- Search across chats (`cmd-f`)
- Pin chats to top
- Track Ollama `context` token arrays to cut resend cost
