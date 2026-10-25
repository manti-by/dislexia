# 03 — UI layout & screens

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

The look is **zcode-like**: dark-first, terminal density, quiet chrome, monospace metadata, an accent-marked assistant column. Built from stock GPUI Kit components — the only custom styling is the palette ([07-theme.md](07-theme.md)) and a handful of layout wrappers.

> Component names below reference the GPUI Kit component library (`gpui_kit::component`). Confirm exact names/props against [gpui-kit docs](https://gpui-kit.com) at implementation time; the mapping is deliberately conservative.

## 1. Main window

```
┌───────────────────────────────────────────────────────────────────────┐
│ ◆ Dislexia — llama3.2:latest                                   ⚙ ⚙   │ ← TitleBar (kit)
├───────────────┬───────────────────────────────────────────────────────┤
│  + New chat   │  you · 14:02                                          │
│               │  ┌─────────────────────────────────────────────────┐ │
│  TODAY        │  │ how do I reverse a Vec in place?                │ │ ← user msg (surface bg)
│  ● rust traits│  └─────────────────────────────────────────────────┘ │
│    gpui theme │  assistant · llama3.2 · 1.2s                         │
│               │  ▍use v.reverse(); — this reverses the Vec in        │ ← assistant msg
│  YESTERDAY    │  ▍place, no reallocation. Note it mutates the…▌      │   (markdown, accent bar)
│    week plan  │                                                       │
│               │                                                       │
│               ├───────────────────────────────────────────────────────┤
│               │ ┌───────────────────────────────────────────────┐     │
│               │ │ Ask anything…  (TextArea, 1–6 rows, grows)   │ [Send] │
│               │ └───────────────────────────────────────────────┘     │
└───────────────┴───────────────────────────────────────────────────────┘
```

### Regions

| Region | Implementation | Notes |
|---|---|---|
| Title bar | kit `TitleBar` | Shows app name + active model; right side: settings icon button, new-chat icon button. Native traffic lights. |
| Sidebar (240px) | `div().flex().flex_col()` + kit `List` | "New chat" button at top; grouped chat list (Today / Yesterday / Older — see 06 §4); bottom: settings entry + server status dot (green/red). Collapsible via `Cmd+B` (stretch). |
| Transcript | kit `List` (virtualized) | Uniform full-width column, max ~72ch readable measure. Auto-scroll to bottom while streaming unless the user scrolled up. |
| Message row | custom light wrapper (see §2) | No chat bubbles on the assistant side — flat blocks, zcode-style. |
| Input area | kit `TextArea` (auto-grow 4–12 rows) inside a floating rounded box at the window bottom; the Send/Stop button is overlaid inside the box's bottom-right corner (zcode-like; changed from the original border-top bar in M7 polish) | `Enter` sends; `Shift+Enter` newline. While streaming the button becomes **Stop** (destructive variant). Disabled (dimmed) while streaming. |

### Message row anatomy

```
user:      <surface panel bg, rounded 6, own max-width, right-aligned to 80% column>
assistant: left 2px accent-colored bar + markdown body (kit Markdown component),
           dim monospace meta line above (model · elapsed seconds), streaming cursor "▌"
error:     assistant-style block with red bar + error text + "Retry" text button
```

Streaming cursor: append `▌` while `status == Streaming`; swap to nothing on completion. Show elapsed time from first chunk to done-chunk (stats arrive in the final chunk — [04](04-ollama-client.md) §5).

## 2. Settings screen

Kit `Modal` centered over a scrim, ~560px wide, sections separated by hairlines. Changes are **saved on every field change** (debounced 300ms) — there is no OK/Cancel in v1.

**Connection**
- Server URL — `Input` (mono font), validated on blur; invalid URL → inline red hint
- Status line + **Test** button → calls `/api/tags`: `● Connected — 5 models` / `● Cannot reach server` (with the Ollama error text)

**Model**
- Model — `Dropdown` populated from `/api/tags` (fetched when the modal opens and via refresh icon)
- `Pull model…` link → opens `http://127.0.0.1:11434` in browser? No — v1 shows a hint row instead ("Run `ollama pull <name>` to add models") to avoid managing pulls.

**Parameters**
- System prompt — `TextArea`, 6 rows, placeholder "No system prompt", **prefilled with the default text-fixer prompt** ([05](05-settings.md) §1.1); a **Reset to default** button restores the embedded prompt
- Temperature — numeric `Input` (0.0–2.0, step 0.1) with slider? v1: numeric input only
- Top P, Top K, Num predict, Num ctx, Repeat penalty — numeric `Input`s, empty = "not sent" (Option)
- Seed — numeric input, empty = random

**General**
- Launch at login — `Switch` (stretch goal, see [08](08-tray-and-lifecycle.md) §8)

All fields map 1:1 to the Settings schema in [05-settings.md](05-settings.md).

## 3. Empty & edge states

| State | Presentation |
|---|---|
| No chats yet | Sidebar shows only "New chat"; transcript area shows centered hint: "Start a conversation with a local model ↓" |
| No model selected | Send disabled with tooltip "Pick a model in Settings"; on first successful connect, auto-select first model from `/api/tags` and toast about it |
| Server unreachable on send | Assistant-style error block: "Can't reach Ollama at `<url>` — is `ollama serve` running?" |
| Model not found (404 from Ollama) | Error block with Ollama's message + "Open Settings" action |
| Empty input | Send disabled |
| Streaming in progress, user opens another chat | Allowed; generation continues writing to its own chat (single in-flight generation still enforced per app, not per view) |

## 4. Interaction details

- Clicking a chat in the sidebar: switches `active_chat_id`, transcript scrolls to bottom, input focused.
- `New chat` clears to a fresh untitled chat (created lazily on first send — see [06](06-chat-history.md) §3).
- Right-click (context menu / hover actions) on a sidebar chat: **Rename**, **Delete** (with confirm popover). v1 minimum: Delete.
- Hover a message row → small copy icon (copies raw text to clipboard via GPUI clipboard API).

## 5. Theming hookup

All colors come from the kit theme (`ActiveTheme`) — screens must contain **no hard-coded colors**; the only hex values live in [07-theme.md](07-theme.md). Typography: UI font default; meta lines `.font_family("mono")` small + dim.

## 6. Sizing & window

- Default window: 1000×680, min 720×480. Remember last frame? — stretch (M7).
- Window title shows active model; set via window titlebar API or TitleBar label.

## 7. Actions & keyboard shortcuts

```rust
actions!(dislexia, [NewChat, OpenSettings, CloseSettings,
                    Send, StopGeneration, FocusInput, HideWindow, QuitApp]);
```

| Shortcut | Action |
|---|---|
| `Enter` (input focused) | Send |
| `Shift+Enter` | Newline |
| `cmd-n` | New chat |
| `cmd-,` | Open settings |
| `esc` | Close settings modal / stop generation (when input empty) |
| `cmd-w` | Hide window (to tray — not quit; see 08) |
| `cmd-q` | Quit (only path besides tray menu) |

Bind via `cx.bind_keys([KeyBinding::new("cmd-n", NewChat, None), ...])` in `main.rs`; dispatch from `Dislexia::handle_action`. Menu-bar equivalent menus are out of scope for v1 (tray menu covers it).
