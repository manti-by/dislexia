# AGENTS.md

Guidance for agents and contributors working on `dislexia` itself.

## What this project is

`dislexia` is a macOS menu-bar chat app for a **local Ollama server**, written in Rust with [GPUI](https://gpui.rs) via [GPUI Kit](https://gpui-kit.com). Its
default persona is a **writing fixer**: the user pastes rough text, the model returns it corrected — spelling and wording fixed, old-fashioned words
modernized, simplified yet more formal. Rust, edition 2024, stable ≥ 1.85.

The implementation contract lives in [BUILD_PLAN.md](BUILD_PLAN.md) and `plan/` — read the matching `plan/0N-*.md` before changing a module. Keep the
plan true when implementation diverges.

## Commands

| Command | Purpose |
| --- | --- |
| `make build` | Build the binary into `target/debug/dislexia`. |
| `make run` | Run the test suite. Unit tests plus the mock-Ollama integration tests; none touch the network. |
| `make check` | `fmt --check`, clippy and tests; the gate before committing. |
| `make app` | Run the same gate, then build `target/release/dislexia` and bundle `target/Dislexia.app`. |
| `make icons` | Regenerate `assets/` (icns + tray template PNGs) from `media/icon-typewriter.svg`. |
| `make clean` | `cargo clean`. |
| `cargo build` | The same build, without the Makefile. |
| `cargo test` | The same tests, without the Makefile. |
| `cargo clippy --all-targets` | Lint. Must stay warning-free. |
| `cargo fmt` | Format. `cargo fmt --check` must be clean. |

Running the app for development is `cargo run` (windowed, logs to stderr via
`RUST_LOG=debug`). It needs a reachable `ollama serve` for chat; UI, settings
and history work without one.

## Layout

| Path | Role |
| --- | --- |
| `src/main.rs` | Boot order only: logging → settings/history load → tray init → open window. No business logic. |
| `src/lib.rs` | Module tree; the testable surface. |
| `src/app.rs` | `Dislexia` root entity: global state (settings, chat store, active chat, streaming status), actions, wiring between UI and services. |
| `src/ollama/` | Pure blocking Rust, **no GPUI imports**: `client.rs` (ureq chat_stream/list_models/test), `types.rs` (serde types, `OllamaError`). Integration-tested against a mock server. |
| `src/settings.rs` | `Settings` schema, defaults (embedded text-fixer prompt), load/save with atomic writes. |
| `src/history.rs` | `ChatStore`: chats in memory, one JSON file per chat, atomic writes, corrupt-file tolerance. |
| `src/theme.rs` | OS appearance observer + the only place colors are assigned (zcode-like palette). |
| `src/ui/` | Rendering and local interaction state only: `sidebar.rs`, `chat_pane.rs`, `message.rs`, `settings_ui.rs`. No HTTP, no file I/O. |
| `src/tray.rs` | Tray icon, menu, event thread; forwards intents to `Dislexia`. Never dropped — dropping removes the icon. |
| `src/platform/macos.rs` | objc2 helpers: Dock activation policy, app activation. Only user: `main.rs` and `tray.rs`. |
| `prompts/default-system-prompt.md` | The default system prompt, embedded with `include_str!`. Product copy; change deliberately. |
| `media/` | Source art. `icon-typewriter.svg` is the single icon source; everything in `assets/` is generated from it. |
| `scripts/` | `make-icons.sh` (SVG → icns + tray PNGs), `bundle.sh` (release build → `target/Dislexia.app`). |
| `tests/ollama.rs` | Integration tests against a hand-written mock Ollama server (`tests/common/`). |

## Conventions

- **The plan is the contract.** `plan/` documents every schema, palette value, error wording and acceptance criterion. When code and plan disagree, either
  fix the code or update the plan in the same change — never leave them diverging silently.
- **`ollama/` stays GPUI-free.** It is pure blocking Rust over `ureq` 3 (no tokio), so it unit-tests and integration-tests without a window. NDJSON is
  line-oriented; the streaming loop is a blocking `BufRead` on GPUI's background executor, checking the cancellation flag between lines.
- **No hard-coded colors outside `theme.rs`.** Views read the GPUI Kit theme (`ActiveTheme`); `src/theme.rs` owns the only hex values. The acceptance gate
  is `rg '#[0-9a-fA-F]{6}' src/ui` → empty.
- **Atomic file writes** for settings and chat files: write `.tmp`, `fs::rename`. Persist chats at message granularity, never per chunk; corrupt chat files
  are renamed `.bad` and skipped, never deleted.
- **User-visible errors live in entities** (a `MessageStatus::Error(String)`), never only in the log, so the UI can always render them.
- Errors in `ollama/` are `thiserror` (`OllamaError`); everything else uses `anyhow` internally. Error wording shown to users comes from the mapping table
  in `plan/04-ollama-client.md` §7 — keep the strings in one place (`ollama/client.rs`).
- Every module that can be tested in isolation carries a `#[cfg(test)] mod tests` block. Stream parsing, request building, settings validation and the
  history store are the high-risk areas and must stay covered.
- Dependencies stay minimal on purpose: `gpui-kit`, `ureq`, `serde`/`serde_json`, `uuid`, `chrono`, `tray-icon`, `objc2*`, `image`, `anyhow`, `thiserror`,
  `log`/`env_logger`. No async runtime.
- GPUI API drift is real: names in `plan/` marked "verify" were checked against the `gpui-kit`-pinned GPUI at M0. When upgrading `gpui-kit`, re-verify
  `Application`, `open_window`, theme init and `component` paths first.

## Working on this repository

Every milestone ends with its acceptance boxes ticked in `plan/10-milestones.md` and `make check` green, then a commit. Milestones build in order M0→M7;
M6 (tray/lifecycle) is the least documented integration — its fallbacks live in `plan/08-tray-and-lifecycle.md` §4.

App data lives in `~/Library/Application Support/dislexia/` (`settings.json`, `chats/<uuid>.json`). The `.app` bundle is ad-hoc signed for local use
(`scripts/bundle.sh`); no `LSUIElement` in Info.plist — Dock visibility is toggled at runtime.
