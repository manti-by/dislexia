# 01 — Architecture

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

## 1. Runtime model

Three execution contexts cooperate:

| Context | Owner | Used for |
|---|---|---|
| **GPUI main runloop** (main thread) | `gpui_kit::Application` | All UI rendering, entity updates, actions, settings/history entity mutation |
| **GPUI background executor** | `cx.background_spawn(...)` | Blocking HTTP to Ollama (`ureq`): streaming reads, model listing, connection tests, file I/O for persistence |
| **Dedicated std thread** (single, daemon-ish) | `src/tray.rs` | Blocks on `tray_icon::TrayIconEvent::receiver()` / `MenuEvent::receiver()` channels and forwards events onto the main runloop |

There is **no tokio/async runtime** in the app. Blocking work is pushed to the background executor; results are applied to GPUI entities via `WeakEntity::update` (or `cx.spawn` futures that resume on the main thread).

> Implementation note: exact signatures (`cx.spawn(async move |this, cx| ...)` shape, `App` vs `Application` naming) must be taken from the **GPUI version pinned by `gpui-kit` 0.8** — check `cargo doc -p gpui-kit --open` / [docs.rs/gpui-kit](https://docs.rs/gpui-kit) when starting M0. GPUI's API still moves; do not copy snippets from older blog posts blindly.

## 2. Module responsibilities

| Module | Owns | Must not do |
|---|---|---|
| `main.rs` | Boot order: logging → load settings → load history → tray init → open window / `Application::run` | No business logic |
| `app.rs` — `Dislexia` root entity | Global state (settings, `ChatStore`, active chat id, streaming status), global actions, wiring between UI and services | No HTTP, no file I/O inline (always via background executor) |
| `ollama/client.rs` | One `OllamaClient { base_url: Url, agent: ureq::Agent }`; `chat_stream`, `list_models`, `test_connection` | No GPUI types — pure blocking Rust, returns typed results (testable without a window) |
| `settings.rs` | `Settings` struct, load/save, defaults | UI |
| `history.rs` | `ChatStore`: in-memory `Vec<ChatMeta>` + active chat bodies; save/load/delete chat files | Ollama calls |
| `theme.rs` | Appearance observer, palette swap | — |
| `ui/*` | Pure rendering + local interaction state; call into `Dislexia` via `Context<Dislexia>` handles | Direct HTTP / file access |
| `tray.rs` | Tray icon lifecycle, menu, event thread | Window logic (forwards intents to `Dislexia`) |
| `platform/macos.rs` | objc2 helpers: dock activation policy, `NSApp` activation | Cross-platform abstraction (macOS-only app; keep the module boundary so a port is thinkable) |

## 3. State ownership

All mutable app state lives in GPUI **entities**; UI reads them and calls `cx.notify()` on change:

```
Dislexia (root, Entity<Dislexia>)
├── settings: Entity<SettingsState>      // schema from 05-settings.md
├── store: Entity<ChatStore>             // model from 06-chat-history.md
│     └── active_chat_id: Option<Uuid>
├── generation: Option<GenerationHandle> // what's streaming now (see 04 §6)
└── ui: UiState                          // modal open? sidebar collapsed? (plain field)
```

Design rules:

- **Single writer per concern**: streaming chunks mutate the chat via `ChatStore` methods only.
- UI never holds a copy of data it can read from an entity — avoids staleness bugs.
- Errors from services are stored **in the entities** (e.g. a `Message::status::Error(String)`), never only logged, so the UI can always show them.

## 4. Sequence: sending a message

```
User types, presses Enter / clicks Send (ui/chat_pane.rs)
  │
  ├─ 1. ChatStore: append Message{role:user}; create placeholder
  │        Message{role:assistant, status:Streaming}; cx.notify()
  ├─ 2. ChatStore: persist chat file (background, fire-and-forget)
  │
  ├─ 3. Dislexia: build ChatRequest from
  │        settings.system_prompt + full message history + settings.model/options
  │
  ├─ 4. cx.background_spawn:
  │        OllamaClient::chat_stream(req)
  │        for each NDJSON line (04 §5):
  │           chat.update(|c| c.append_chunk(chunk)); cx.notify()
  │           check cancellation flag between lines
  │
  ├─ 5a. done-chunk → mark status Complete (keep stats), persist chat file
  ├─ 5b. Err(e)     → mark status Error(msg_from(e)); persist; cx.notify()
  │
  └─ UI re-renders incrementally (transcript scrolls to bottom)
```

Cancellation (Stop button) sets a flag on the `GenerationHandle`; the background loop checks it between lines and drops the connection (`ureq` read timeout bounds the worst-case wait — see [04-ollama-client.md](04-ollama-client.md) §6).

## 5. Error strategy

- `ollama/types.rs` defines `OllamaError` (thiserror): `Connection`, `Timeout`, `Http { status, body }`, `Api { message }` (Ollama `"error"` field), `Io`, `Parse`.
- Everything else uses `anyhow` internally; **user-visible errors** are always one of: connection-trouble wording, model-not-found wording, or the raw Ollama error string — mapping table in [04-ollama-client.md](04-ollama-client.md) §7.

## 6. Crate dependency direction

```
main → app → { ui, tray, theme }
            ↘ ollama (no gpui types except none at all — keep it gpui-free)
            ↘ settings, history (gpui entities, serde-only data)
platform/macos ← used only by main.rs and tray.rs
```

`ollama` must stay free of GPUI imports so it can be integration-tested with a plain mock server ([09-testing.md](09-testing.md)).
