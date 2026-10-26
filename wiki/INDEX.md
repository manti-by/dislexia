# Dislexia Wiki - Index

Knowledge base for the Dislexia project - one Markdown page per debugging chase,
investigation, code review, or set of changes, plus the reference pages that hold
the build plan. See [README.md](README.md) for conventions and
[TEMPLATE.md](TEMPLATE.md) for the page template. New pages are created or updated
on demand with `/wiki-update`; the plugin only scaffolds the wiki.

## Pages

_Newest first. All pages currently share the date 2026-10-26; the two session pages come first,
then the build plan reads in order._

- [UI polish: greyscale theme, resizable sidebar, tray icon](pages/2026-10-26-ui-polish-greyscale-theme.md) — Fixed window chrome, wrapping, tray icon, resizable sidebar and switched to a greyscale palette. (2026-10-26)
- [Spiral icons and window frame persistence](pages/2026-10-26-spiral-icons-window-frame.md) — Replaced the typewriter mark with the `media/dislexia.svg` spiral via a new `svg-glyph.py` normalization step, and made the window remember its frame per display. (2026-10-26)
- [Dislexia — Build Plan](pages/2026-10-26-build-plan.md) — What the app is, the tech stack, the module map, the eight decisions made, milestone summary. (2026-10-26)
- [01 — Architecture](pages/2026-10-26-01-architecture.md) — Module map, state ownership, runtime model, send-a-message sequence. (2026-10-26)
- [02 — Project setup](pages/2026-10-26-02-project-setup.md) — Cargo project, dependencies, toolchain, `.app` bundling, icon pipeline, dev workflow. (2026-10-26)
- [03 — UI layout & screens](pages/2026-10-26-03-ui-layout.md) — Window regions, message anatomy, settings dialog, empty states, shortcuts. (2026-10-26)
- [04 — Ollama client](pages/2026-10-26-04-ollama-client.md) — Endpoints, request/response schemas, NDJSON streaming loop, cancellation, error taxonomy. (2026-10-26)
- [05 — Settings](pages/2026-10-26-05-settings.md) — Settings schema, defaults (embedded text-fixer prompt), validation, atomic storage, propagation. (2026-10-26)
- [06 — Chat history](pages/2026-10-26-06-chat-history.md) — Chat/message model, per-chat JSON files, title derivation, sidebar grouping, replay rules. (2026-10-26)
- [07 — Theme (OS light/dark, zcode-like palette)](pages/2026-10-26-07-theme.md) — OS appearance sync, greyscale palette values, tray template images, contrast checks. (2026-10-26)
- [08 — System tray & app lifecycle](pages/2026-10-26-08-tray-and-lifecycle.md) — Tray icon and menu, event pump, Dock policy, close-to-background, quit paths. (2026-10-26)
- [09 — Testing](pages/2026-10-26-09-testing.md) — Unit and mock-server integration tests, manual QA checklist, the CI gate. (2026-10-26)
- [10 — Milestones & build order](pages/2026-10-26-10-milestones.md) — M0–M7 scope and acceptance criteria, dependency graph, risk register. (2026-10-26)
- [GPUI Kit API-shape notes (verified at M0)](pages/2026-10-26-api-notes.md) — The pinned GPUI Kit names the app relies on; re-verify on upgrade. (2026-10-26)

## By topic

### Build plan & project setup

- [Dislexia — Build Plan](pages/2026-10-26-build-plan.md)
- [01 — Architecture](pages/2026-10-26-01-architecture.md)
- [02 — Project setup](pages/2026-10-26-02-project-setup.md)
- [09 — Testing](pages/2026-10-26-09-testing.md)
- [10 — Milestones & build order](pages/2026-10-26-10-milestones.md)

### UI, theme & window

- [03 — UI layout & screens](pages/2026-10-26-03-ui-layout.md)
- [07 — Theme (OS light/dark, zcode-like palette)](pages/2026-10-26-07-theme.md)
- [UI polish: greyscale theme, resizable sidebar, tray icon](pages/2026-10-26-ui-polish-greyscale-theme.md)
- [Spiral icons and window frame persistence](pages/2026-10-26-spiral-icons-window-frame.md)

### Chat backend & storage

- [04 — Ollama client](pages/2026-10-26-04-ollama-client.md)
- [05 — Settings](pages/2026-10-26-05-settings.md)
- [06 — Chat history](pages/2026-10-26-06-chat-history.md)

### Lifecycle & platform

- [08 — System tray & app lifecycle](pages/2026-10-26-08-tray-and-lifecycle.md)
- [GPUI Kit API-shape notes (verified at M0)](pages/2026-10-26-api-notes.md)
