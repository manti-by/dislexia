# 09 — Testing

Back to [BUILD_PLAN.md](../BUILD_PLAN.md).

The high-risk areas are stream parsing, request building, and persistence — all conveniently testable without a GUI. Tray/GPUI integration is covered by a manual QA checklist (§3).

## 1. Unit tests (in-crate, `cargo test`)

| Area | File | Cases |
|---|---|---|
| NDJSON parsing | `ollama/types.rs` | delta line → `ChatChunk::Delta` with content; done line → stats parsed; mid-stream `{"error": "…"}` surfaces; empty `message.content` default; garbage line → `Parse` error; untagged enum tolerates missing stats fields |
| Request building | `ollama/client.rs` | system prompt prepended iff non-empty; `Error`/`Streaming` messages excluded from replay ([06](06-chat-history.md) §6); `options` object omitted entirely when all-None; Option fields skipped individually; model name round-trips |
| Settings | `settings.rs` | defaults on missing file; serde round-trip; unknown fields ignored; URL normalization (trailing slash, scheme-less input); out-of-range clamps rejected |
| History store | `history.rs` | create-append-persist-reload round-trip (`tempfile::tempdir`); title derivation (48-char truncation, first line only, fences stripped); corrupt file skipped without panic and renamed `.bad`; delete removes file; sort/group keys (Today/Yesterday/Older boundaries via injected clock or chrono test values) |
| Error mapping | `ollama/client.rs` | 404 → model-not-found wording; refused → connection wording — pure fn over `OllamaError` |

Fixture NDJSON lines live inline as `const` strings or `tests/fixtures/*.ndjson`.

## 2. Integration tests (`tests/ollama.rs`) — mock Ollama

Spin a minimal HTTP server on a std thread (`TcpListener` + hand-written HTTP/1.1 responses — no extra dependency) that serves canned payloads, then exercise `OllamaClient` against `http://127.0.0.1:<ephemeral>`:

- `GET /api/tags` → canned model list; assert names parse.
- `POST /api/chat` → chunked NDJSON (write headers, flush line-by-line, small sleeps) → assert iterator yields deltas then a done chunk with stats; assert final assembled text.
- Mid-stream error line → `OllamaError::Api`.
- Immediate 404 body `{"error":"model 'x' not found"}` → mapped wording.
- Abrupt socket close → `Io`/`Parse` error, not a hang (read timeout sized small in tests).
- Cancellation: set the flag after first delta; iterator returns `Cancelled` before consuming all lines.

This mock is ~80 lines of Rust; keep it in `tests/common/mod.rs` for reuse.

## 3. Manual QA checklist (run before each milestone merge; full pass at M7)

**Chat**
- [ ] Send message with server up → user block appears instantly, assistant streams token-by-token, cursor disappears at end, elapsed + stats shown
- [ ] Multi-turn: follow-up references earlier answer (history replay works)
- [ ] Stop mid-stream → partial text kept, "(stopped)" meta, can send again
- [ ] Server down → friendly error block; start `ollama serve`; next send works
- [ ] Unknown model → model-not-found error + hint
- [ ] Enter sends / Shift+Enter newline / empty input → Send disabled
- [ ] Copy button puts exact message text on the clipboard

**Settings**
- [ ] Change port/host → status dot + Test reflect reality; model dropdown repopulates
- [ ] System prompt change visible in next request (verify via `ollama` server logs or a mock)
- [ ] Temperature etc. round-trip through app restart (file inspection)

**History**
- [ ] Quit + relaunch → all chats, titles, grouping intact; streaming-completed chat shows full text
- [ ] Delete chat asks, disappears from sidebar, file gone from disk
- [ ] New chat discarded without sending → no file created

**Theme**
- [ ] Toggle System appearance (Settings → Appearance) while app running → instant swap, both palettes legible
- [ ] Launch while system is in Light → starts light
- [ ] Tray icon visible/correct in both menu-bar appearances (template rendering)

**Tray / lifecycle**
- [ ] Launch → Dock icon present + tray icon present
- [ ] `cmd-w` → window gone, Dock icon gone, tray alive, process alive after 60s
- [ ] Generation running → close window → reopen: generation completed/persisted
- [ ] Tray left-click → window opens focused (Accessory → front-most)
- [ ] Tray "New Chat" → opens window with fresh chat, input focused
- [ ] Tray "Quit" and `cmd-q` both exit cleanly; no orphan tray icon

**Packaging**
- [ ] `scripts/bundle.sh` output launches; behaves identically to `cargo run` except no console

## 4. CI-ish hygiene (local)

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Run on every milestone completion; no CI configured in v1 (add GitHub Actions when a remote exists — keep the three commands as the job).
