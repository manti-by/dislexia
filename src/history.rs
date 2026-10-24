//! Chat history data model and store (`plan/06-chat-history.md`).
//!
//! `ChatStore` is a plain struct — the app hosts it inside a GPUI entity and
//! calls `cx.notify()` after mutations; keeping the methods context-free
//! makes the lifecycle unit-testable without a window. Persistence to
//! `chats/<uuid>.json` is layered on in M4. Appending messages is the single
//! writer path the streaming loop goes through.

use std::cmp::Reverse;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One chat transcript on disk and in memory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chat {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
}

/// A single message; exactly the content replayed to Ollama (`plan/06 §6`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub status: MessageStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// Live-cycle state of a message. Errors are user-visible data, never only
/// logged; `Error`/live `Streaming` messages are excluded from API replay.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageStatus {
    Complete,
    Streaming {
        started_at: DateTime<Utc>,
        model: String,
    },
    /// Cancelled by the user; partial text is kept.
    Stopped,
    /// The stream failed; carries the user-facing wording (`plan/04 §7`).
    Error {
        message: String,
    },
}

impl MessageStatus {
    pub fn is_streaming(&self) -> bool {
        matches!(self, MessageStatus::Streaming { .. })
    }

    /// Model name recorded with the message, if any.
    pub fn model(&self) -> Option<&str> {
        match self {
            MessageStatus::Streaming { model, .. } => Some(model),
            _ => None,
        }
    }
}

/// In-memory chat list plus the active chat id (the sidebar's selection).
/// When `dir` is set, chat files live in it — one JSON per chat.
#[derive(Debug)]
pub struct ChatStore {
    chats: Vec<Chat>,
    active_chat_id: Option<Uuid>,
    dir: Option<std::path::PathBuf>,
}

impl Default for ChatStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatStore {
    /// Starts with one fresh untitled chat, active — the app never renders
    /// without a transcript (`plan/06 §3`, launch row).
    pub fn new() -> Self {
        let mut store = Self {
            chats: Vec::new(),
            active_chat_id: None,
            dir: None,
        };
        store.new_chat();
        store
    }

    /// Like [`Self::new`], but chat files persist in `dir`.
    pub fn new_in(dir: &std::path::Path) -> Self {
        let mut store = Self::new();
        store.dir = Some(dir.to_owned());
        store
    }

    /// Loads every chat file in `dir` (`plan/06 §2–3`): newest first, active
    /// chat = newest. A fresh in-memory chat starts when there are none.
    /// Unparseable files are renamed `<uuid>.json.bad` — logged, skipped,
    /// never deleted.
    pub fn load(dir: &std::path::Path) -> Self {
        let mut store = Self {
            chats: Vec::new(),
            active_chat_id: None,
            dir: Some(dir.to_owned()),
        };

        let mut loaded: Vec<Chat> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|extension| extension != "json") {
                    continue;
                }
                match std::fs::read_to_string(&path)
                    .map_err(anyhow::Error::from)
                    .and_then(|text| Ok(serde_json::from_str::<Chat>(&text)?))
                {
                    Ok(chat) => loaded.push(chat),
                    Err(error) => {
                        log::warn!("skipping corrupt chat file {}: {error:#}", path.display());
                        let bad = path.with_extension("json.bad");
                        if let Err(rename_error) = std::fs::rename(&path, &bad) {
                            log::warn!("cannot rename {}: {rename_error}", path.display());
                        }
                    }
                }
            }
        }
        loaded.sort_by_key(|chat| std::cmp::Reverse(chat.updated_at));
        if loaded.is_empty() {
            store.new_chat();
        } else {
            store.active_chat_id = Some(loaded[0].id);
            store.chats = loaded;
        }
        store
    }

    /// Creates a fresh untitled chat and makes it active.
    pub fn new_chat(&mut self) -> Uuid {
        let now = Utc::now();
        let chat = Chat {
            id: Uuid::new_v4(),
            title: String::new(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        };
        let id = chat.id;
        self.chats.push(chat);
        self.active_chat_id = Some(id);
        id
    }

    /// Removes a chat from memory and deletes its file, if any
    /// (`plan/06 §3`). The active selection moves to the newest remaining
    /// chat (a fresh untitled one when the store empties).
    pub fn delete_chat(&mut self, id: Uuid) {
        self.chats.retain(|chat| chat.id != id);
        if let Some(dir) = &self.dir {
            let _ = remove_chat_file(dir, id);
        }
        self.active_chat_id = None;
        if let Some(newest) = self.sorted_chats().first().map(|chat| chat.id) {
            self.active_chat_id = Some(newest);
        }
        if self.active_chat_id.is_none() {
            self.new_chat();
        }
    }

    /// The chat's file path, when persistence is on.
    pub fn chat_file(&self, id: Uuid) -> Option<std::path::PathBuf> {
        Some(self.dir.as_ref()?.join(format!("{id}.json")))
    }

    pub fn chats(&self) -> &[Chat] {
        &self.chats
    }

    /// Chats sorted newest-first — the sidebar order (`plan/06 §5`).
    pub fn sorted_chats(&self) -> Vec<&Chat> {
        let mut chats: Vec<&Chat> = self.chats.iter().collect();
        chats.sort_by_key(|chat| Reverse(chat.updated_at));
        chats
    }

    pub fn active_chat_id(&self) -> Option<Uuid> {
        self.active_chat_id
    }

    pub fn active_chat(&self) -> Option<&Chat> {
        self.active_chat_id.and_then(|id| self.chat(id))
    }

    pub fn chat(&self, id: Uuid) -> Option<&Chat> {
        self.chats.iter().find(|chat| chat.id == id)
    }

    pub fn set_active(&mut self, id: Uuid) -> bool {
        if self.chat(id).is_some() {
            self.active_chat_id = Some(id);
            true
        } else {
            false
        }
    }

    /// Appends the just-typed user message and derives the title from the
    /// first one (`plan/06 §3–4`).
    pub fn append_user_message(&mut self, chat_id: Uuid, text: &str) {
        let Some(chat) = self.chat_mut(chat_id) else {
            return;
        };
        chat.updated_at = Utc::now();
        chat.messages.push(Message {
            role: Role::User,
            content: text.to_owned(),
            created_at: chat.updated_at,
            status: MessageStatus::Complete,
        });
        if chat.title.is_empty() {
            chat.title = derive_title(text);
        }
    }

    /// Appends the empty assistant message the stream writes into.
    pub fn append_assistant_placeholder(&mut self, chat_id: Uuid, model: &str) {
        let Some(chat) = self.chat_mut(chat_id) else {
            return;
        };
        chat.updated_at = Utc::now();
        chat.messages.push(Message {
            role: Role::Assistant,
            content: String::new(),
            created_at: chat.updated_at,
            status: MessageStatus::Streaming {
                started_at: chat.updated_at,
                model: model.to_owned(),
            },
        });
    }

    /// Appends one streamed delta to the live assistant message. No-ops when
    /// the chat has no streaming message (the single-writer guarantee).
    pub fn append_chunk(&mut self, chat_id: Uuid, text: &str) {
        let Some(chat) = self.chat_mut(chat_id) else {
            return;
        };
        let Some(last) = chat.messages.last_mut() else {
            return;
        };
        if last.role == Role::Assistant && last.status.is_streaming() {
            last.content.push_str(text);
        }
    }

    /// Marks the streaming message Complete (or Stopped when cancelled) and
    /// bumps the chat's sort key.
    pub fn finish_streaming(&mut self, chat_id: Uuid, stopped: bool) {
        let Some(chat) = self.chat_mut(chat_id) else {
            return;
        };
        chat.updated_at = Utc::now();
        if let Some(last) = chat.messages.last_mut()
            && last.status.is_streaming()
        {
            last.status = if stopped {
                MessageStatus::Stopped
            } else {
                MessageStatus::Complete
            };
        }
    }

    /// Marks the streaming message as failed with the user-facing wording
    /// from the error mapping (`plan/04 §7`).
    pub fn fail_streaming(&mut self, chat_id: Uuid, error: String) {
        let Some(chat) = self.chat_mut(chat_id) else {
            return;
        };
        chat.updated_at = Utc::now();
        if let Some(last) = chat.messages.last_mut()
            && last.status.is_streaming()
        {
            last.status = MessageStatus::Error { message: error };
        }
    }

    pub fn is_streaming(&self, chat_id: Uuid) -> bool {
        self.chat(chat_id).is_some_and(|chat| {
            chat.messages
                .last()
                .is_some_and(|message| message.status.is_streaming())
        })
    }

    fn chat_mut(&mut self, id: Uuid) -> Option<&mut Chat> {
        self.chats.iter_mut().find(|chat| chat.id == id)
    }
}

/// The chats directory: `~/Library/Application Support/dislexia/chats/`.
pub fn chats_dir() -> std::path::PathBuf {
    crate::settings::data_dir().join("chats")
}

/// Writes one chat file atomically (`plan/06 §2`): tmp + rename.
pub fn save_chat(dir: &std::path::Path, chat: &Chat) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(chat)?;
    let path = dir.join(format!("{}.json", chat.id));
    let tmp = dir.join(format!("{}.json.tmp", chat.id));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

fn remove_chat_file(dir: &std::path::Path, id: Uuid) -> std::io::Result<()> {
    match std::fs::remove_file(dir.join(format!("{id}.json"))) {
        Ok(()) => Ok(()),
        // Already gone is fine — deleting an untitled in-memory chat.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Sidebar group header for a chat by `updated_at`, local time
/// (`plan/06 §4`): Today / Yesterday / Previous 7 days / Older.
pub fn group_label(
    updated_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> &'static str {
    use chrono::{Duration, Local, TimeZone};

    let local_date = Local
        .from_utc_datetime(&updated_at.naive_utc())
        .date_naive();
    let today = Local.from_utc_datetime(&now.naive_utc()).date_naive();
    let days_apart = (today - local_date).num_days();
    if local_date == today {
        "Today"
    } else if days_apart == 1 {
        "Yesterday"
    } else if (Duration::days(1)..Duration::days(8)).contains(&Duration::days(days_apart.max(0))) {
        "Previous 7 days"
    } else {
        "Older"
    }
}

/// Sidebar title from the first user message: first line, whitespace
/// collapsed, markdown fences stripped, cut at 48 chars (`plan/06 §4`).
pub fn derive_title(text: &str) -> String {
    let first_line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.starts_with("```"))
        .unwrap_or("");
    let title = first_line.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.chars().count() > 48 {
        let cut: String = title.chars().take(48).collect();
        format!("{cut}…")
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_from_first_line() {
        assert_eq!(derive_title("hello\nworld"), "hello");
        assert_eq!(derive_title("  a   b\t c  "), "a b c");
        assert_eq!(derive_title("```\ncode\n```"), "code");
    }

    #[test]
    fn title_truncated_at_48_chars() {
        let long = "x".repeat(60);
        let title = derive_title(&long);
        assert_eq!(title.chars().count(), 49); // 48 + the ellipsis
        assert!(title.ends_with('…'));

        let exact = "y".repeat(48);
        assert_eq!(derive_title(&exact), exact);
    }

    #[test]
    fn title_of_multibyte_text_counts_chars() {
        let text = "ж".repeat(60);
        let title = derive_title(&text);
        assert_eq!(title.chars().count(), 49);
    }

    #[test]
    fn append_and_status_lifecycle() {
        let mut store = ChatStore::new();
        let chat_id = store.active_chat_id().unwrap();

        store.append_user_message(chat_id, "fix this");
        store.append_assistant_placeholder(chat_id, "llama3.2");
        assert_eq!(store.chat(chat_id).unwrap().title, "fix this");
        assert!(store.is_streaming(chat_id));

        store.append_chunk(chat_id, "here ");
        store.append_chunk(chat_id, "you go");
        assert_eq!(
            store.chat(chat_id).unwrap().messages[1].content,
            "here you go"
        );

        store.finish_streaming(chat_id, false);
        assert!(!store.is_streaming(chat_id));
        assert!(matches!(
            store.chat(chat_id).unwrap().messages[1].status,
            MessageStatus::Complete
        ));
    }

    #[test]
    fn chunk_appends_are_ignored_without_a_streaming_tail() {
        let mut store = ChatStore::new();
        let chat_id = store.active_chat_id().unwrap();
        store.append_user_message(chat_id, "hi");
        store.append_chunk(chat_id, "orphan");
        assert_eq!(store.chat(chat_id).unwrap().messages.len(), 1);
    }

    #[test]
    fn stopped_and_error_statuses() {
        let mut store = ChatStore::new();
        let chat_id = store.active_chat_id().unwrap();

        store.append_user_message(chat_id, "hi");
        store.append_assistant_placeholder(chat_id, "m");
        store.finish_streaming(chat_id, true);
        assert!(matches!(
            store.chat(chat_id).unwrap().messages[1].status,
            MessageStatus::Stopped
        ));

        store.append_user_message(chat_id, "again");
        store.append_assistant_placeholder(chat_id, "m");
        store.fail_streaming(chat_id, "boom".into());
        assert!(matches!(
            store.chat(chat_id).unwrap().messages[3].status,
            MessageStatus::Error { ref message } if message == "boom"
        ));
    }

    #[test]
    fn sorted_chats_are_newest_first() {
        let mut store = ChatStore::new();
        let first = store.active_chat_id().unwrap();
        let second = store.new_chat();

        let sorted = store.sorted_chats();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].id, second);
        assert_eq!(sorted[1].id, first);
    }

    #[test]
    fn set_active_switches_selection() {
        let mut store = ChatStore::new();
        let first = store.active_chat_id().unwrap();
        let second = store.new_chat();

        assert!(store.set_active(first));
        assert_eq!(store.active_chat_id(), Some(first));
        assert!(!store.set_active(Uuid::nil()));
        assert_eq!(store.active_chat_id(), Some(first));
        assert_ne!(first, second);
    }

    #[test]
    fn chat_store_round_trips_through_files() {
        let dir = tempfile::tempdir().unwrap();

        let mut store = ChatStore::new();
        let first = store.active_chat_id().unwrap();
        let second = store.new_chat();
        store.append_user_message(first, "first chat");
        store.append_user_message(second, "second chat\nmore text");
        for chat in store.chats() {
            save_chat(dir.path(), chat).unwrap();
        }

        let reloaded = ChatStore::load(dir.path());
        assert_eq!(reloaded.chats().len(), 2);
        assert_eq!(reloaded.active_chat_id(), Some(second));
        let loaded_second = reloaded.chat(second).unwrap();
        assert_eq!(loaded_second.title, "second chat");
        assert_eq!(loaded_second.messages[0].content, "second chat\nmore text");
    }

    #[test]
    fn corrupt_chat_file_is_renamed_bad_and_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let corrupt = dir.path().join("00000000-0000-0000-0000-000000000001.json");
        std::fs::write(&corrupt, "{not a chat").unwrap();

        let store = ChatStore::load(dir.path());
        assert_eq!(store.chats().len(), 1, "fresh untitled chat");
        assert!(
            dir.path()
                .join("00000000-0000-0000-0000-000000000001.json.bad")
                .exists()
        );
        assert!(!corrupt.exists());
    }

    #[test]
    fn delete_chat_removes_memory_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ChatStore::new_in(dir.path());
        let first = store.active_chat_id().unwrap();
        let second = store.new_chat();
        store.append_user_message(first, "one");
        store.append_user_message(second, "two");
        for chat in store.chats() {
            save_chat(dir.path(), chat).unwrap();
        }
        assert!(dir.path().join(format!("{first}.json")).exists());

        store.delete_chat(first);
        assert_eq!(store.chats().len(), 1);
        assert!(!dir.path().join(format!("{first}.json")).exists());
        assert_eq!(store.active_chat_id(), Some(second));

        // Deleting the last remaining chat falls back to a fresh one.
        store.delete_chat(second);
        assert_eq!(store.chats().len(), 1);
        assert_ne!(store.active_chat_id(), Some(first));
    }

    #[test]
    fn sidebar_groups_by_local_date() {
        use chrono::TimeZone;
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 10, 24, 18, 0, 0)
            .unwrap();
        let today = chrono::Utc
            .with_ymd_and_hms(2026, 10, 24, 0, 30, 0)
            .unwrap();
        let yesterday = chrono::Utc
            .with_ymd_and_hms(2026, 10, 23, 18, 0, 0)
            .unwrap();
        let three_days_ago = chrono::Utc
            .with_ymd_and_hms(2026, 10, 21, 12, 0, 0)
            .unwrap();
        let long_ago = chrono::Utc.with_ymd_and_hms(2026, 9, 1, 12, 0, 0).unwrap();

        assert_eq!(group_label(today, now), "Today");
        assert_eq!(group_label(yesterday, now), "Yesterday");
        assert_eq!(group_label(three_days_ago, now), "Previous 7 days");
        assert_eq!(group_label(long_ago, now), "Older");
    }

    #[test]
    fn message_statuses_serialize_with_tags() {
        let status = MessageStatus::Streaming {
            started_at: chrono::Utc::now(),
            model: "llama3.2".into(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains(r#""type":"streaming""#), "{json}");
        let back: MessageStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model(), Some("llama3.2"));

        let error = serde_json::to_string(&MessageStatus::Error {
            message: "x".into(),
        })
        .unwrap();
        assert!(error.contains(r#""type":"error""#), "{error}");
        assert!(error.contains(r#""message":"x""#));
        let stopped = serde_json::to_string(&MessageStatus::Stopped).unwrap();
        assert!(stopped.contains(r#""type":"stopped""#));
    }
}
