# GPUI Kit API-shape notes (verified at M0)

Checked against `gpui-kit = "0.8"` (longbridge/gpui-kit, crates/kit 0.8.0) — the
pinned GPUI behind it. Record of the exact names the app relies on; re-verify
on any `gpui-kit` upgrade (plan/10 risk #3).

## Application / window

- Boot: `gpui_kit::application().run(|cx: &mut App| { ... })` (no `Application::new()`).
- Layer init: `gpui_kit::init(cx)` — runs `gpui_component::init` (theme registry, inputs).
- Entity creation on `App`: `cx.new(|cx| View)` — requires `use gpui_kit::*;`
  (the `AppContext` trait comes with the glob).
- Open window: `gpui_kit::open_window(options, cx, |window, cx| -> Entity<V>)
  -> Result<(AnyWindowHandle, Entity<V>)>` — wraps the view in a `base::Root`
  so dialogs/sheets/menus work. Note: build closure returns the **app view**,
  not the Root.
- Title bar: `gpui_kit::component::TitleBar` (a `ParentElement`; children render
  left of the window controls). Use `TitleBar::window_options()` as the base of
  the `WindowOptions` (`app_owns_titlebar_drag: true`, transparent titlebar,
  traffic-light position 9,9).
- `WindowOptions.window_min_size: Some(Size { width: px(720.), height: px(480.) })`.

## Theme

- `gpui_kit::component::ActiveTheme` trait → `cx.theme() -> &Theme` (works on
  `App` and contexts). Deref target `ThemeColor` has `background`, `foreground`,
  `primary`, `border`, `danger`, `muted`, `muted_foreground`, `sidebar`, …
- Palette edits: `Theme::update(cx, |theme| theme.colors.x = …)` (keeps tokens +
  Base projection + all windows in step — do NOT write `global_mut`).
- Mode switch / OS sync: `Theme::change(mode, window, cx)` and
  `Theme::sync_system_appearance(window, cx)`; `ThemeMode::from(WindowAppearance)`.
- Live OS switch: `cx.observe_window_appearance(window, |this, window, cx| …)`.

## Components (gpui_kit::component::*)

- Layout helpers: `h_flex()`, `v_flex()`.
- Input: `Input::new(&Entity<InputState>)`; state `InputState::new(window, cx)`
  (from `gpui_base::input`, re-exported via `component::input`). Events:
  `InputEvent::{Change, PressEnter { secondary, shift }, Focus, Blur}` via
  `cx.subscribe(&state, …)`. Value: `state.read(cx).value()`;
  `set_value(text, window, cx)`; `state.focus(window, cx)`.
- Textarea: `Textarea::new(&Entity<TextareaState>)`,
  `TextareaState::new(window, cx).auto_grow(1, 6)`; `TextareaMode`/auto-grow
  rows are built in.
- Button: `Button::new(id).label(..).on_click(|_, window, cx| ..)`, variants
  (`.primary()`, `.danger()`, `.ghost()`, `.small()`), `Sizable` trait.
- Streaming markdown: `TextViewState::markdown(initial, cx)` (an `Entity`) +
  `state.update(cx, |s, cx| s.push_str(chunk, cx))`, render with
  `TextView::new(&state)`; `ScrollHandle::scroll_to_bottom()` for follow.
- Dialog (ex-Modal): `component::dialog::Dialog` — `.trigger(el)`,
  `.content(|cx| el)`, `.title(..)`, `.w(px)`, `.overlay_closable(true)`.
- Select (ex-Dropdown): `component::select::{Select, SelectState}` —
  `SelectState::new(...)` + `set_items(...)`; `Select::new(&state)`
  `.placeholder(..)`, searchable.
- Icons: `Icon::new(IconName::Settings)` etc. from the default `assets` crate.
- Clipboard: `window.write_to_clipboard(ClipboardItem::new_string(text))` (GPUI core).

## Toolchain

- stable 1.99.0 (≥ 1.85 required), edition 2024. Third-party `block v0.1.6`
  emits a future-incompat warning (objc2 dep chain) — known, not actionable.
