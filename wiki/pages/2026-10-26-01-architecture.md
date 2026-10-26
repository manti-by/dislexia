---
title: 01 — Architecture
date: 2026-10-26
type: reference
status: reference
session_id: -
services: [app, ui, ollama]
branch: master
tickets: []
tags: [plan, architecture, concurrency, gpui]
related: [2026-10-26-02-project-setup.md, 2026-10-26-03-ui-layout.md, 2026-10-26-04-ollama-client.md, 2026-10-26-05-settings.md, 2026-10-26-06-chat-history.md, 2026-10-26-07-theme.md, 2026-10-26-08-tray-and-lifecycle.md, 2026-10-26-09-testing.md, 2026-10-26-build-plan.md]
---

# 01 — Architecture

Part of the build plan — start at [[2026-10-26-build-plan]].

## TL;DR

Three execution contexts, one root entity. `main.rs` boots, `Dislexia`
owns settings, chats and the in-flight generation, `ollama/` stays GPUI-free and
runs on the background executor, and the tray forwards events from its own thread
onto the main runloop. No tokio anywhere.

---

## 1. Runtime model

Three execution contexts cooperate:

| Context | Owner | Used for |
|---|---|---|
| **GPUI main runloop** (main thread) | `gpui_kit::Application` | All UI rendering, entity updates, actions, settings/history mutation, and the tray event pump |
| **GPUI background executor** | `cx.background_spawn(...)` | Blocking HTTP to Ollama (`ureq`): streaming reads, model listing, connection tests, file I/O for persistence |
| **GPUI foreground task** (`src/tray.rs`) | `cx.spawn(...)` + a 100ms timer | Drains `tray_icon::TrayIconEvent` / `MenuEvent` channels and dispatches intents on the main thread |

There is **no tokio/async runtime** in the app. Blocking work is pushed to the background executor;
results are applied to GPUI entities via `WeakEntity::update` (or `cx.spawn` futures that resume on
the main thread).

> Implementation note (M6): the tray does **not** need a dedicated std thread. A foreground task
> polling both global receivers every 100ms keeps every entity access on the main thread by
> construction; the poll is invisible at menu-click cadence. See
> [[2026-10-26-08-tray-and-lifecycle]] §3.

## 2. Module responsibilities

| Module | Owns | Must not do |
|---|---|---|
| `main.rs` | Boot order: logging → theme → key bindings → tray → settings load → open window / `application().run` | No business logic |
| `app.rs` — `Dislexia` root entity | Global state (settings, `ChatStore`, active chat id, streaming status), global actions, wiring between UI and services | No HTTP, no file I/O inline (always via background executor) |
| `ollama/client.rs` | One `OllamaClient { base: String, agent: ureq::Agent }`; `chat`, `list_models`, `test_connection` | No GPUI types — pure blocking Rust, returns typed results (testable without a window) |
| `settings.rs` | `Settings` struct, load/save, defaults | UI |
| `history.rs` | `ChatStore`: in-memory `Vec<Chat>`, active chat id; save/load/delete chat files | Ollama calls |
| `theme.rs` | OS appearance sync, palette registration | — |
| `ui/*` | Pure rendering + local interaction state; call into `Dislexia` via `Context<Dislexia>` handles | Direct HTTP / file access |
| `tray.rs` | Tray icon lifecycle, menu, event pump | Window logic (forwards intents to `Dislexia`) |
| `platform/macos.rs` | objc2 helpers: dock activation policy, `NSApp` activation, native traffic-light hiding | Cross-platform abstraction (macOS-only app; keep the module boundary so a port is thinkable) |

## 3. State ownership

All mutable app state hangs off the root `Dislexia` entity; UI reads it and calls `cx.notify()` on
change:

```
Dislexia (root, Entity<Dislexia>)
├── settings: Settings                    // plain field, schema from [[2026-10-26-05-settings]] §1
├── store: Entity<ChatStore>              // model from [[2026-10-26-06-chat-history]]
│     └── active_chat_id: Option<Uuid>
├── input: Entity<TextareaState>          // the composer
├── settings_ui: SettingsUi               // dialog editors + connection status
├── generation: Option<GenerationHandle>  // what's streaming now (see [[2026-10-26-04-ollama-client]] §6)
└── window: Option<AnyWindowHandle>       // captured on first render
```

The window's root view entity is owned by the window, so process-wide handles live in an `AppRoot`
global (weak root + window handle) for the tray to reach — see
[[2026-10-26-08-tray-and-lifecycle]] §4.

Design rules:

- **Single writer per concern**: streaming chunks mutate the chat via `ChatStore` methods only.
- UI never holds a copy of data it can read from an entity — avoids staleness bugs.
- Errors from services are stored **in the entities** (e.g. `MessageStatus::Error { message }`), never only logged, so the UI can always show them.

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
  ├─ 4. cx.background_executor().spawn:
  │        client.chat(&request, &cancel)  → ChatStream
  │        for each NDJSON line ([[2026-10-26-04-ollama-client]] §5):
  │           chat.update(|c| c.append_chunk(chunk)); cx.notify()
  │           ChatStream checks `cancel` between lines
  │
  ├─ 5a. done-chunk → mark status Complete (keep stats), persist chat file
  ├─ 5b. Err(e)     → mark status Error { message: user_error_message(e) }; persist; cx.notify()
  │
  └─ UI re-renders incrementally (transcript scrolls to bottom)
```

Cancellation (Stop button / `esc`) calls `CancelFlag::cancel()`; `ChatStream` checks it between
lines and yields `Err(OllamaError::Cancelled)`, which the caller turns into `MessageStatus::Stopped`
(partial text kept). `ureq`'s read timeout bounds the worst-case wait — see
[[2026-10-26-04-ollama-client]] §6.

## 5. Error strategy

- `ollama/types.rs` defines `OllamaError` (thiserror): `Connection`, `Timeout`, `Http { status, body }`, `Api { message }` (Ollama `"error"` field), `Io`, `Parse`.
- Everything else uses `anyhow` internally; **user-visible errors** are always one of: connection-trouble wording, model-not-found wording, or the raw Ollama error string — mapping table in [[2026-10-26-04-ollama-client]] §7.

## 6. Crate dependency direction

```
main → app → { ui, tray, theme }
            ↘ ollama (no gpui types except none at all — keep it gpui-free)
            ↘ settings, history (gpui entities, serde-only data)
platform/macos ← used by main.rs, app.rs and tray.rs (never by ui/)
```

`ollama` must stay free of GPUI imports so it can be integration-tested with a plain mock server ([[2026-10-26-09-testing]]).

---

## Follow-ups

- None — the module boundaries are enforced in review; update this page when a module takes on a new responsibility.
- Consistency check 2026-10-26: the tray hop is a GPUI foreground task (not a std thread); client and state-tree names now match `src/ollama/`, `src/app.rs` and `src/tray.rs`.

## References

- Related: [[2026-10-26-02-project-setup]], [[2026-10-26-03-ui-layout]],
  [[2026-10-26-04-ollama-client]], [[2026-10-26-05-settings]],
  [[2026-10-26-06-chat-history]], [[2026-10-26-07-theme]],
  [[2026-10-26-08-tray-and-lifecycle]], [[2026-10-26-09-testing]],
  [[2026-10-26-build-plan]]
- Code: `src/app.rs`, `src/ollama/`, `src/tray.rs`, `src/platform/macos.rs`
