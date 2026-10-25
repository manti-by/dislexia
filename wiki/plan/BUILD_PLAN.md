# Dislexia — Build Plan

A macOS menu-bar chat app for local Ollama models, written in Rust with [GPUI](https://gpui.rs) (via [GPUI Kit](https://gpui-kit.com)).

> Status: **plan** — nothing is implemented yet. This document set is the recipe for implementation. Work through the [milestones](plan/10-milestones.md) in order.

---

## 1. What we are building

Dislexia is a lightweight native macOS client that talks to a **local Ollama server**. Its default persona is a **writing fixer**: the user pastes rough text, the model returns it corrected — spelling and wording fixed, old-fashioned words modernized, simplified yet more formal.

- A chat window with a **message input and a Send button**. Sending a message compiles a request from:
  - **System prompt** — configured in Settings; the default (the text-fixer prompt at [default-system-prompt.md](../../prompts/default-system-prompt.md)) is embedded at compile time
  - **User prompt** — from the input field (plus prior conversation history)
  - **Model & model options** (temperature, top_p, …) — configured in Settings
  - **Server URL** — configured in Settings (default `http://127.0.0.1:11434`)
- Responses **stream token-by-token** into the transcript.
- A **list of previous chats** in a sidebar; chats are persisted across launches.
- The app **lives in the system tray**. Closing the window hides it from the Dock and keeps it running in the background; it is always reachable from the tray icon.
- The **theme follows the OS** (light/dark) and switches live. Visual language mimics **zcode**: dark-first, terminal-like density, minimal chrome — built from stock GPUI Kit components with almost no custom styling.

### Non-goals (v1)

- No streaming of multiple concurrent chats (one in-flight generation at a time)
- No remote Ollama auth/TLS beyond what `ureq` gives by default
- No iOS/Linux/Windows targets (crate choices are macOS-first)
- No prompt library / tool calling / multimodal inputs

---

## 2. Tech stack

| Concern         | Choice                                                                 | Why |
|-----------------|------------------------------------------------------------------------|-----|
| UI framework    | [`gpui-kit = "0.8"`](https://crates.io/crates/gpui-kit)                | One dependency that re-exports a pinned GPUI (`use gpui_kit::*` **is** GPUI) plus ~75 stock components (input, textarea, button, list, sidebar, modal, dropdown, markdown, title bar). Matches "GPUI components without a lot of customizations". |
| HTTP client     | [`ureq = "3"`](https://crates.io/crates/ureq) (blocking)                | Ollama streams NDJSON; a blocking reader on GPUI's background executor is the simplest, tokio-free design. Trivially unit-testable. |
| System tray     | [`tray-icon = "0.26"`](https://github.com/tauri-apps/tray-icon)         | De-facto Rust tray crate; works alongside GPUI's macOS runloop (see [08-tray-and-lifecycle.md](plan/08-tray-and-lifecycle.md)). |
| macOS platform  | `objc2` + `objc2-app-kit`                                              | Dock activation policy (hide/show Dock icon), app activation when reopening from tray. |
| Persistence     | `serde` + `serde_json`, plain files in `~/Library/Application Support/dislexia/` | No DB dependency; chats are small; human-debuggable. |
| IDs / time      | `uuid`, `chrono`                                                       | |

Toolchain: **stable Rust ≥ 1.85, edition 2024** (required by `ureq` 3 / kit deps).

---

## 3. Architecture at a glance

```
┌─────────────────────────── macOS ────────────────────────────┐
│  GPUI main runloop (main thread)                             │
│  ┌───────────────────── Dislexia (root Entity) ────────────┐ │
│  │  AppState: settings, chat store, active chat, UI state  │ │
│  │  UI tree: Sidebar | ChatPane | SettingsModal            │ │
│  └──────┬───────────────────────────────┬──────────────────┘ │
│         │ cx.background_spawn           │ entity updates      │
│  ┌──────▼──────────────┐   ┌───────────▼──────────────────┐  │
│  │ Background executor │   │ Tray events thread (std)     │  │
│  │ ureq POST /api/chat │   │ tray-icon channels → forward │  │
│  │ NDJSON line reader  │   │ into main runloop            │  │
│  └──────┬──────────────┘   └──────────────────────────────┘  │
└─────────┼─────────────────────────────────────────────────────┘
          ▼
   Ollama server (default http://127.0.0.1:11434)
```

Details: [01-architecture.md](plan/01-architecture.md).

---

## 4. Document index

| Doc | Contents |
|-----|----------|
| [01-architecture.md](01-architecture.md) | Module map, state ownership, concurrency model, send-a-message sequence |
| [02-project-setup.md](02-project-setup.md) | Cargo project, dependencies, toolchain, `.app` bundling, Info.plist, dev workflow |
| [03-ui-layout.md](03-ui-layout.md) | Window layout, screens, component mapping (zcode-like look), shortcuts, empty states |
| [04-ollama-client.md](04-ollama-client.md) | Endpoints, request/response schemas, NDJSON streaming loop, cancellation, error taxonomy |
| [05-settings.md](05-settings.md) | Settings schema, defaults, storage format, settings UI fields |
| [06-chat-history.md](06-chat-history.md) | Chat/message data model, persistence format, sidebar listing, lifecycle |
| [07-theme.md](07-theme.md) | OS appearance tracking, light/dark palette (zcode-like), tray icon adaptation |
| [08-tray-and-lifecycle.md](08-tray-and-lifecycle.md) | Tray icon + menu, close-to-background, Dock icon policy, reopen & quit flows, known risks |
| [09-testing.md](09-testing.md) | Unit/integration test plan (mock Ollama server), manual QA checklist |
| [10-milestones.md](10-milestones.md) | Build order M0–M7 with acceptance criteria, risk register |

---

## 5. Future source layout

```
dislexia/
├── Cargo.toml
├── README.md                   # repo/app docs — content checklist in 02-project-setup.md §9, written in M7
├── scripts/
│   ├── bundle.sh              # builds Dislexia.app (see 02-project-setup.md)
│   └── make-icons.sh          # media/icon-typewriter.svg → assets/ (icns + tray PNGs)
├── media/                     # source art (see media/README.md)
│   ├── icon-typewriter.svg    # the icon: monochrome typewriter (MDI, Apache-2.0)
│   └── icon-preview-512.png   # preview render, not used by the app
├── prompts/
│   └── default-system-prompt.md  # default Settings system prompt, embedded via include_str!
├── assets/                    # all generated from media/icon-typewriter.svg — do not hand-edit
│   ├── icon.icns
│   ├── tray-16@1x.png         # template images (see 07-theme.md)
│   └── tray-32@2x.png
├── src/
│   ├── main.rs                # entry: Application::run, tray init, dock policy
│   ├── app.rs                 # Dislexia root entity, global actions/state
│   ├── settings.rs            # Settings struct, load/save (05)
│   ├── history.rs             # ChatStore: load/save/list chats (06)
│   ├── theme.rs               # appearance observer + palette (07)
│   ├── ollama/
│   │   ├── mod.rs
│   │   ├── client.rs          # ureq client: chat_stream/list_models/test (04)
│   │   └── types.rs           # serde types for the Ollama API
│   ├── ui/
│   │   ├── mod.rs             # window/workspace composition
│   │   ├── sidebar.rs         # previous-chats list
│   │   ├── chat_pane.rs       # transcript + input + send/stop
│   │   ├── message.rs         # one message row
│   │   └── settings_ui.rs     # settings modal
│   ├── tray.rs                # tray icon, menu, event forwarding (08)
│   └── platform/
│       └── macos.rs           # objc2: activation policy, activate app
```

---

## 6. Decisions already made

1. **GPUI via `gpui-kit`**, not raw `gpui` from crates.io (0.2.2 lags far behind the API the kit pins; the kit also supplies the text input, which raw GPUI does not ship).
2. **Blocking `ureq` on GPUI's background executor** instead of embedding a tokio runtime. One less runtime; NDJSON is a line-oriented protocol, a blocking `BufRead` loop is a perfect fit.
3. **One chat file per chat** (`chats/<uuid>.json`) instead of one big JSON — cheap atomic writes, no index to corrupt.
4. **Runtime Dock-icon toggling** (`NSApplicationActivationPolicy::Regular` ↔ `Accessory`) instead of `LSUIElement=true` in Info.plist, so the app still looks like a normal app while its window is open.
5. **Streaming always on** (`"stream": true`); no non-streaming code path.
6. State is chat-sent in full every turn (Ollama `/api/chat` is stateless); we do **not** track Ollama `context` tokens in v1.
7. **App icon**: the minimal monochrome **typewriter** glyph [`media/icon-typewriter.svg`](media/icon-typewriter.svg) (Material Design Icons, Apache-2.0). Every icon the app uses — bundle `.icns` and tray template PNGs — is generated from this single SVG by `scripts/make-icons.sh` (see [media/README.md](media/README.md) and [02-project-setup.md](02-project-setup.md) §7).
8. **Default system prompt** = the text-fixer prompt at [`prompts/default-system-prompt.md`](prompts/default-system-prompt.md), embedded with `include_str!` and used as the Settings default ([05-settings.md](05-settings.md) §1.1). User edits always win; the app never overwrites a saved prompt.

## 7. Milestone summary

| # | Milestone | Depends on |
|---|-----------|------------|
| M0 | Project scaffold: window opens with GPUI Kit theme | — |
| M1 | UI skeleton: sidebar + chat pane + input/send (fake echo responses) | M0 |
| M2 | Ollama client wired in: real streaming chat against local server | M0, M1 |
| M3 | Settings screen + persistence + model list from `/api/tags` | M2 |
| M4 | Chat history persistence + sidebar of previous chats | M1 |
| M5 | Theme: follow OS light/dark, live switch | M1 |
| M6 | Tray + background lifecycle (close → tray, reopen, quit) | M0 |
| M7 | Polish & packaging: `.app` bundle, icon, shortcuts, copy, QA pass | all |

Detailed scope and acceptance criteria: [10-milestones.md](10-milestones.md).
