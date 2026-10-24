# Dislexia

Dislexia is a macOS menu-bar chat app for a **local Ollama server**. Its
default persona is a **writing fixer**: paste rough text, get it back
corrected — spelling and wording fixed, old-fashioned words modernized,
simplified yet formal.

Built in Rust with [GPUI](https://gpui.rs) via [GPUI Kit](https://gpui-kit.com); the visual language is dark-first and terminal-like
(zcode-ish), built from stock components.

## Features

- **Streaming chat** against a local Ollama server — responses render
  token-by-token, with a Stop button that keeps the partial text.
- **Text-fixer system prompt by default** ([prompts/default-system-prompt.md](prompts/default-system-prompt.md)), editable in Settings; your edits always win.
- **Chat history** — one JSON file per chat, sidebar grouped
  Today / Yesterday / Previous 7 days / Older, delete with confirm.
- **Settings** (`cmd-,`) — server URL with connection test, model dropdown
  from `/api/tags`, temperature / top-p / top-k / context options; changes
  take effect on the very next send.
- **Lives in the tray** — closing the window (`cmd-w` or the red button)
  keeps the app running without a Dock icon; the tray icon reopens it,
  starts a new chat, or quits. In-flight generations finish in the
  background.
- **Follows the OS light/dark appearance** live.

## Requirements

- macOS 13+
- Rust stable ≥ 1.85 (to build)
- A local Ollama: `ollama serve` plus at least one pulled model, e.g.
  `ollama pull llama3.2`

## Development

```
make build        # target/debug/dislexia
make run          # the test suite (unit + mock-Ollama integration; no network)
make check        # fmt --check + clippy + tests — the gate
RUST_LOG=debug cargo run   # windowed, logs to stderr
```

The app talks to `http://127.0.0.1:11434` by default; UI, settings and
history work without a running Ollama, chat needs one.

## Build & package

```
make icons        # regenerate assets/ from media/icon-typewriter.svg
make app          # the check gate, then bundle target/Dislexia.app
open target/Dislexia.app
```

The bundle is ad-hoc signed for local use.

## Data

App data lives in `~/Library/Application Support/dislexia/`:
`settings.json` and `chats/<uuid>.json` (one file per chat; corrupt files
are renamed `.bad`, never deleted).

## Credits & license

- Icon: "typewriter" from [Material Design Icons](https://pictogrammers.com/library/mdi/icon/typewriter/) (Apache-2.0), single source for the
  app icon and the tray template.
- UI: [GPUI Kit](https://gpui-kit.com) and the GPUI it pins.
- License: BSD-3-Clause.

## Architecture

The implementation contract lives in [BUILD_PLAN.md](BUILD_PLAN.md) and
`plan/` — module map, schemas, palette, error wording, milestones and the
verified GPUI Kit API shapes (plan/api-notes.md). [AGENTS.md](AGENTS.md)
summarizes the working conventions.
