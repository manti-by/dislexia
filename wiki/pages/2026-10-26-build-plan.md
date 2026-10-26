---
title: Dislexia — Build Plan
date: 2026-10-26
type: reference
status: reference
session_id: -
services: [app]
branch: master
tickets: []
tags: [plan, overview, decisions]
related: [2026-10-26-01-architecture.md, 2026-10-26-02-project-setup.md, 2026-10-26-03-ui-layout.md, 2026-10-26-04-ollama-client.md, 2026-10-26-05-settings.md, 2026-10-26-06-chat-history.md, 2026-10-26-07-theme.md, 2026-10-26-08-tray-and-lifecycle.md, 2026-10-26-09-testing.md, 2026-10-26-10-milestones.md, 2026-10-26-api-notes.md]
---

# Dislexia — Build Plan

This page is the entry point for the build-plan set; the numbered pages
(01–10) expand it, and [[2026-10-26-api-notes]] records the verified API
shapes.

## TL;DR

The index and contract for the whole app: what Dislexia is, the tech stack,
the module map, the eight decisions already made and the milestone summary. Every
other page in this set expands one section of it.

---

A macOS menu-bar chat app for local Ollama models, written in Rust with [GPUI](https://gpui.rs) (via [GPUI Kit](https://gpui-kit.com)).

> Status: **M0–M5 complete, M6–M7 open.** These pages are the implementation
> contract: read the matching page before changing a module, and keep it true
> when implementation diverges.

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
| UI framework    | [`gpui-kit = "0.8"`](https://crates.io/crates/gpui-kit)                | One dependency that re-exports a pinned GPUI (`use gpui_kit::*` **is** GPUI) plus ~75 stock components (input, textarea, button, dialog, select, text view, title bar). Matches "GPUI components without a lot of customizations". |
| HTTP client     | [`ureq = "3"`](https://crates.io/crates/ureq) (blocking)                | Ollama streams NDJSON; a blocking reader on GPUI's background executor is the simplest, tokio-free design. Trivially unit-testable. |
| System tray     | [`tray-icon = "0.26"`](https://github.com/tauri-apps/tray-icon)         | De-facto Rust tray crate; works alongside GPUI's macOS runloop (see [[2026-10-26-08-tray-and-lifecycle]]). |
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

Details: [[2026-10-26-01-architecture]].

---

## 4. Document index

| Doc | Contents |
|-----|----------|
| [[2026-10-26-01-architecture]] | Module map, state ownership, concurrency model, send-a-message sequence |
| [[2026-10-26-02-project-setup]] | Cargo project, dependencies, toolchain, `.app` bundling, Info.plist, dev workflow |
| [[2026-10-26-03-ui-layout]] | Window layout, screens, component mapping (zcode-like look), shortcuts, empty states |
| [[2026-10-26-04-ollama-client]] | Endpoints, request/response schemas, NDJSON streaming loop, cancellation, error taxonomy |
| [[2026-10-26-05-settings]] | Settings schema, defaults, storage format, settings UI fields |
| [[2026-10-26-06-chat-history]] | Chat/message data model, persistence format, sidebar listing, lifecycle |
| [[2026-10-26-07-theme]] | OS appearance tracking, light/dark palette (zcode-like), tray icon adaptation |
| [[2026-10-26-08-tray-and-lifecycle]] | Tray icon + menu, close-to-background, Dock icon policy, reopen & quit flows, known risks |
| [[2026-10-26-09-testing]] | Unit/integration test plan (mock Ollama server), manual QA checklist |
| [[2026-10-26-10-milestones]] | Build order M0–M7 with acceptance criteria, risk register |

---

## 5. Source layout

```
dislexia/
├── Cargo.toml
├── Makefile                    # build / run / check / app / icons / clean
├── README.md                   # repo/app docs — checklist in 02-project-setup.md §9
├── .github/
│   ├── workflows/              # ci.yml (the gate) + release.yml (version bump → release)
│   └── scripts/               # one shell script per workflow step, shellcheck-linted
├── scripts/
│   ├── bundle.sh              # builds Dislexia.app (see 02-project-setup.md)
│   ├── make-icons.sh          # media/dislexia.svg → assets/ (icns + tray PNGs)
│   ├── svg-glyph.py           # normalize the art: black ink, square, padded
│   └── png-template.py        # opaque raster → black + alpha template
├── media/                     # source art (see media/README.md)
│   └── dislexia.svg           # the icon: white spiral stroke on transparent
├── prompts/
│   └── default-system-prompt.md  # default Settings system prompt, embedded via include_str!
├── assets/                    # all generated from media/dislexia.svg — do not hand-edit
│   ├── icon.icns
│   ├── tray-16@1x.png         # template images (see 07-theme.md)
│   └── tray-32@2x.png
├── src/
│   ├── main.rs                # entry: application().run, tray init, dock policy
│   ├── app.rs                 # Dislexia root entity, global actions/state
│   ├── settings.rs            # Settings struct, load/save (05)
│   ├── history.rs             # ChatStore: load/save/list chats (06)
│   ├── theme.rs               # OS appearance sync + palette (07)
│   ├── ollama/
│   │   ├── mod.rs
│   │   ├── client.rs          # ureq client: chat/list_models/test (04)
│   │   └── types.rs           # serde types for the Ollama API
│   ├── ui/
│   │   ├── mod.rs             # window options + composition
│   │   ├── title_bar.rs       # custom 12px window controls
│   │   ├── sidebar.rs         # previous-chats list
│   │   ├── chat_pane.rs       # transcript + input + send/stop
│   │   ├── message.rs         # one message row
│   │   └── settings_ui.rs     # settings dialog
│   ├── tray.rs                # tray icon, menu, event pump (08)
│   └── platform/
│       ├── mod.rs
│       └── macos.rs           # objc2: activation policy, activate app, hide buttons
└── tests/
    ├── ollama.rs              # mock-Ollama integration tests (09)
    └── common/mod.rs
```

---

## 6. Decisions already made

1. **GPUI via `gpui-kit`**, not raw `gpui` from crates.io (0.2.2 lags far behind the API the kit pins; the kit also supplies the text input, which raw GPUI does not ship).
2. **Blocking `ureq` on GPUI's background executor** instead of embedding a tokio runtime. One less runtime; NDJSON is a line-oriented protocol, a blocking `BufRead` loop is a perfect fit.
3. **One chat file per chat** (`chats/<uuid>.json`) instead of one big JSON — cheap atomic writes, no index to corrupt.
4. **Runtime Dock-icon toggling** (`NSApplicationActivationPolicy::Regular` ↔ `Accessory`) instead of `LSUIElement=true` in Info.plist, so the app still looks like a normal app while its window is open.
5. **Streaming always on** (`"stream": true`); no non-streaming code path.
6. State is chat-sent in full every turn (Ollama `/api/chat` is stateless); we do **not** track Ollama `context` tokens in v1.
7. **App icon**: the hand-drawn **spiral** stroke [`media/dislexia.svg`](../../media/dislexia.svg). Every icon the app uses — bundle `.icns` and tray template PNGs — is generated from this single SVG by `scripts/make-icons.sh` (see [media/README.md](../../media/README.md) and [[2026-10-26-02-project-setup]] §7).
8. **Default system prompt** = the text-fixer prompt at [`prompts/default-system-prompt.md`](../../prompts/default-system-prompt.md), embedded with `include_str!` and used as the Settings default ([[2026-10-26-05-settings]] §1.1). User edits always win; the app never overwrites a saved prompt.

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

Detailed scope and acceptance criteria: [[2026-10-26-10-milestones]].

---

## Follow-ups

- Status is M0–M5 complete, M6–M7 open — keep the status line and §7 in step as milestones land.
- Decision #8 is product copy: the default prompt may change deliberately, never by accident.
- Consistency check 2026-10-26: §5 now lists the shipped tree (`Makefile`, `.github/`, `svg-glyph.py`, `title_bar.rs`, `tests/`).

## References

- Related: [[2026-10-26-01-architecture]], [[2026-10-26-02-project-setup]],
  [[2026-10-26-03-ui-layout]], [[2026-10-26-04-ollama-client]],
  [[2026-10-26-05-settings]], [[2026-10-26-06-chat-history]], [[2026-10-26-07-theme]],
  [[2026-10-26-08-tray-and-lifecycle]], [[2026-10-26-09-testing]],
  [[2026-10-26-10-milestones]], [[2026-10-26-api-notes]]
