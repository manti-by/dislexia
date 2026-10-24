//! The `Dislexia` root entity: global state (settings, chat store, active
//! chat, streaming status), actions and wiring between UI and services.
//!
//! See `plan/01-architecture.md`. No HTTP and no file I/O here — blocking
//! work always goes through the background executor into the service
//! modules.

use std::sync::Arc;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::Icon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::{IconName, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::*;

use uuid::Uuid;

use crate::history::ChatStore;
use crate::ollama::{
    self, CancelFlag, ChatChunk, ChatRequest, OllamaClient, OllamaError, Options,
    user_error_message,
};
use crate::settings::{self, Settings};
use crate::ui;
use crate::ui::settings_ui::{self, ConnectionStatus, SettingsUi};

actions!(
    dislexia,
    [
        NewChat,
        Send,
        StopGeneration,
        OpenSettings,
        FocusInput,
        HideWindow,
        QuitApp
    ]
);

/// Debounce for settings file writes (`plan/05 §4`).
const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

/// Commits one integer option from its editor text; `None` for empty
/// ("not sent"), the field name is recorded on a parse error.
fn commit_int<T: std::str::FromStr>(
    raw: String,
    field: &'static str,
    errors: &mut Vec<&'static str>,
) -> Option<T> {
    if raw.is_empty() {
        return None;
    }
    match raw.parse::<T>() {
        Ok(value) => Some(value),
        Err(_) => {
            errors.push(field);
            None
        }
    }
}

/// Cancellation handle for the in-flight generation (`plan/04 §6`). Stored
/// on the root entity while a stream runs; the background loop checks the
/// flag between lines.
#[derive(Clone)]
pub struct GenerationHandle {
    pub cancel: CancelFlag,
    pub chat_id: Uuid,
    #[allow(dead_code)] // elapsed meta line lands with stats display (M7)
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// Process-wide handles to the root entity and its window. The root view
/// entity is owned by its window, so when the window closes the handle goes
/// weak and the tray recreates the window (reloading state from disk,
/// `plan/08 §4`).
#[derive(Default)]
pub struct AppRoot {
    pub root: Option<WeakEntity<Dislexia>>,
    pub window: Option<AnyWindowHandle>,
}

impl Global for AppRoot {}

/// Shows (or recreates) the main window and brings it to the front
/// (`plan/08 §4`).
pub fn show_main_window(cx: &mut App) {
    let state = cx.global::<AppRoot>();
    let alive = state
        .window
        .is_some_and(|handle| cx.windows().contains(&handle));
    if alive {
        if let Some(handle) = state.window {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
    } else {
        // Recreate: settings and chats reload from disk.
        let _ = ui::open_main_window(ui::window_options(cx), settings::load(), cx);
    }
    cx.activate(true);
    crate::platform::macos::set_dock_policy(crate::platform::macos::DockPolicy::Regular);
    crate::platform::macos::activate_app();
}

/// Tray "New Chat": show the window, then start a fresh chat in it
/// (`plan/08 §2`).
pub fn new_chat_from_tray(cx: &mut App) {
    show_main_window(cx);
    let (root, window) = {
        let state = cx.global::<AppRoot>();
        (state.root.clone(), state.window)
    };
    if let (Some(root), Some(window)) = (root, window) {
        let _ = window.update(cx, |_, window, cx| {
            root.update(cx, |this, cx| this.handle_new_chat(window, cx))
                .ok();
        });
    }
}

/// The root entity; every screen reads state from here.
pub struct Dislexia {
    store: Entity<ChatStore>,
    input: Entity<TextareaState>,
    /// Scroll handle of the transcript, for follow-on-append.
    scroll: ScrollHandle,
    /// Chat settings (`plan/05`); persisted, edited in the settings dialog.
    settings: Settings,
    /// Editors + status for the settings dialog (rendered by settings_ui).
    pub(crate) settings_ui: SettingsUi,
    /// Bumps on every settings edit; the last writer wins the file write.
    save_generation: u64,
    /// Streaming generation, if any (one in-flight at a time, `plan/01 §1`).
    generation: Option<GenerationHandle>,
    /// True while the input holds only whitespace — Send is disabled then.
    input_empty: bool,
    /// Handle of the main window, captured on first render; needed to touch
    /// window-bound components (dialog editors) from background tasks.
    window: Option<AnyWindowHandle>,
    _subscriptions: Vec<Subscription>,
}

impl Dislexia {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_settings(Settings::default(), window, cx)
    }

    pub fn with_settings(settings: Settings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.new(|_| ChatStore::load(&crate::history::chats_dir()));
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 6)
                .submit_on_enter(true)
                .placeholder("Paste text to fix, or ask anything…")
        });
        let settings_ui = settings_ui::build_state(&settings, window, cx);

        let subscriptions = vec![cx.subscribe_in(
            &input,
            window,
            |this, input, event: &InputEvent, window, cx| match event {
                // Enter sends (submit_on_enter turns Enter into the action;
                // Shift+Enter still inserts a newline).
                InputEvent::PressEnter { shift, .. } if !shift => {
                    this.send(window, cx);
                    input.update(cx, |state, cx| state.focus(window, cx));
                }
                InputEvent::PressEnter { .. } => {}
                InputEvent::Change => {
                    this.input_empty = input.read(cx).value().trim().is_empty();
                    cx.notify();
                }
                _ => {}
            },
        )];

        // Every settings field commits on change; persistence is debounced.
        for state in [
            settings_ui.server_url.clone(),
            settings_ui.temperature.clone(),
            settings_ui.top_p.clone(),
            settings_ui.top_k.clone(),
            settings_ui.num_predict.clone(),
            settings_ui.num_ctx.clone(),
            settings_ui.repeat_penalty.clone(),
            settings_ui.seed.clone(),
        ] {
            cx.subscribe(&state, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.commit_settings(cx);
                }
            })
            .detach();
        }
        cx.subscribe(
            &settings_ui.system_prompt,
            |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.commit_settings(cx);
                }
            },
        )
        .detach();
        cx.subscribe(
            &settings_ui.model,
            |this, _, event: &SelectEvent<Vec<SharedString>>, cx| {
                let SelectEvent::Confirm(value) = event;
                this.settings.model = value.as_ref().map(|value| {
                    // The "missing on server" marker is presentation only.
                    value.trim_end_matches(" (missing on server)").to_owned()
                });
                this.schedule_save(cx);
                cx.notify();
            },
        )
        .detach();

        let mut this = Self {
            store,
            input,
            scroll: ScrollHandle::new(),
            settings,
            settings_ui,
            save_generation: 0,
            generation: None,
            input_empty: true,
            window: None,
            _subscriptions: subscriptions,
        };
        this.first_run_check(cx);
        this
    }

    /// First-run experience (`plan/05 §6`): without a stored model, probe
    /// `/api/tags` and auto-select the first model.
    fn first_run_check(&mut self, cx: &mut Context<Self>) {
        let probe_needed = self.settings.model.is_none();
        self.refresh_models(cx);
        if !probe_needed {
            return;
        }
        cx.spawn(async move |this, cx| {
            let first = probe_first_model(&this, cx).await;
            this.update(cx, |this, cx| {
                if let Some(first) = first
                    && this.settings.model.is_none()
                {
                    this.settings.model = Some(first.clone());
                    this.schedule_save(cx);
                    log::info!("first run: auto-selected model {first}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn store(&self) -> &Entity<ChatStore> {
        &self.store
    }

    /// The composer state (chat pane renders the Textarea from it).
    pub fn input(&self) -> &Entity<TextareaState> {
        &self.input
    }

    /// The scroll handle backing the transcript (chat pane reads it).
    pub fn scroll_handle(&self) -> &ScrollHandle {
        &self.scroll
    }

    pub fn is_streaming(&self) -> bool {
        self.generation.is_some()
    }

    pub fn input_empty(&self) -> bool {
        self.input_empty
    }

    /// The model label for the title bar.
    pub fn model_label(&self) -> String {
        self.settings
            .model
            .clone()
            .unwrap_or_else(|| "no model".to_owned())
    }

    /// Handle for callbacks inside the settings dialog.
    pub fn weak_handle(&self, cx: &Context<Self>) -> WeakEntity<Self> {
        cx.entity().downgrade()
    }

    pub fn handle_new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.store.update(cx, |store, _| store.new_chat());
        cx.notify();
        self.focus_input(window, cx);
    }

    pub fn handle_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.send(window, cx);
    }

    pub fn handle_stop(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = &self.generation {
            handle.cancel.cancel();
            cx.notify();
        }
    }

    /// Focuses the composer; used on chat switch, new chat and window open.
    pub fn focus_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.input.clone();
        window.defer(cx, move |window, cx| {
            input.update(cx, |state, cx| state.focus(window, cx));
        });
    }

    /// Re-reads every settings editor into `self.settings`, records field
    /// errors and schedules the debounced save (`plan/05 §3–4`).
    fn commit_settings(&mut self, cx: &mut Context<Self>) {
        let raw_server_url = self.settings_ui.server_url.read(cx).value().to_string();
        match settings::normalize_server_url(&raw_server_url) {
            Ok(url) => {
                self.settings.server_url = url;
                self.settings_ui.url_error = None;
            }
            Err(message) => self.settings_ui.url_error = Some(message),
        }

        self.settings.system_prompt = self.settings_ui.system_prompt.read(cx).value().to_string();

        let mut errors: Vec<&'static str> = Vec::new();
        let read = |state: &Entity<InputState>| state.read(cx).value().trim().to_string();

        // Bounded f64s and plain integers. An invalid field keeps its last
        // good value and gets an inline hint (`plan/05 §3`); empty means
        // "not sent".
        let commit_f64 = |raw: String,
                          min: f64,
                          max: f64,
                          field: &'static str,
                          errors: &mut Vec<&'static str>| {
            if raw.is_empty() {
                return None;
            }
            match raw
                .parse::<f64>()
                .ok()
                .filter(|value| (*value >= min) && (*value <= max))
            {
                Some(value) => Some(value),
                None => {
                    errors.push(field);
                    None
                }
            }
        };
        let ui = &self.settings_ui;
        let options = &mut self.settings.options;
        options.temperature = commit_f64(read(&ui.temperature), 0., 2., "temperature", &mut errors);
        options.top_p = commit_f64(read(&ui.top_p), 0., 1., "top_p", &mut errors);
        options.repeat_penalty = commit_f64(
            read(&ui.repeat_penalty),
            0.,
            3.,
            "repeat_penalty",
            &mut errors,
        );
        options.top_k = commit_int(read(&ui.top_k), "top_k", &mut errors);
        options.num_predict = commit_int::<i32>(read(&ui.num_predict), "num_predict", &mut errors);
        options.num_ctx = commit_int(read(&ui.num_ctx), "num_ctx", &mut errors);
        options.seed = commit_int::<i64>(read(&ui.seed), "seed", &mut errors);

        self.settings_ui.option_errors = errors;
        self.schedule_save(cx);
        cx.notify();
    }

    /// Restores the embedded text-fixer prompt (`plan/05 §1.1`).
    pub fn reset_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default = settings::DEFAULT_SYSTEM_PROMPT.to_owned();
        self.settings.system_prompt = default.clone();
        self.settings_ui.system_prompt.update(cx, |state, cx| {
            state.set_value(&default, window, cx);
        });
        self.schedule_save(cx);
        cx.notify();
    }

    /// Writes settings to disk after `SAVE_DEBOUNCE` of quiet; the write
    /// itself runs on the background executor (`plan/05 §4`).
    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.save_generation += 1;
        let generation = self.save_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            this.update(cx, |this, cx| {
                if this.save_generation != generation {
                    return; // superseded by a newer edit
                }
                let settings = this.settings.clone();
                cx.background_executor()
                    .spawn(async move {
                        if let Err(error) = settings::save(&settings) {
                            log::error!("cannot save settings: {error:#}");
                        }
                    })
                    .detach();
            })
            .ok();
        })
        .detach();
    }

    /// `GET /api/tags` in the background; repopulates the model dropdown
    /// (`plan/05 §5`).
    pub fn refresh_models(&mut self, cx: &mut Context<Self>) {
        let server_url = self.settings.server_url.clone();
        cx.spawn(async move |this, cx| {
            let result = list_models_background(&server_url, cx).await;
            this.update(cx, |this, cx| match result {
                Ok(models) => {
                    this.settings_ui.models = models;
                    this.sync_model_select(cx);
                    cx.notify();
                }
                Err(error) => log::debug!("cannot list models: {error:?}"),
            })
            .ok();
        })
        .detach();
    }

    /// Connection test against `/api/tags` (`plan/05 §6`).
    pub fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.status = ConnectionStatus::Testing;
        cx.notify();

        let server_url = self.settings.server_url.clone();
        cx.spawn(async move |this, cx| {
            let result = list_models_background(&server_url, cx).await;
            this.update(cx, |this, cx| {
                this.settings_ui.status = match &result {
                    Ok(models) => ConnectionStatus::Connected(models.len()),
                    Err(error) => ConnectionStatus::Failed(user_error_message(
                        error,
                        &this.settings.server_url,
                        this.settings.model.as_deref().unwrap_or(""),
                    )),
                };
                if let Ok(models) = &result {
                    this.settings_ui.models = models.clone();
                    this.sync_model_select(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Persists one chat at message granularity (`plan/06 §3`): after the
    /// user message and when a stream finishes — never per chunk. Chats
    /// without messages never create a file; the write itself is
    /// fire-and-forget on the background executor.
    pub fn persist_chat(&mut self, chat_id: Uuid, cx: &mut Context<Self>) {
        let Some(chat) = self.store.read(cx).chat(chat_id).cloned() else {
            return;
        };
        if chat.messages.is_empty() {
            return;
        }
        let dir = crate::history::chats_dir();
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = crate::history::save_chat(&dir, &chat) {
                    log::warn!("cannot persist chat {}: {error:#}", chat.id);
                }
            })
            .detach();
    }

    /// Deletes a chat (sidebar, confirm dialog): drops it from memory and
    /// removes the file in the background (`plan/06 §3`).
    pub fn handle_delete_chat(&mut self, chat_id: Uuid, cx: &mut Context<Self>) {
        self.store.update(cx, |store, _| store.delete_chat(chat_id));
        cx.notify();
    }

    /// Pushes `settings_ui.models` into the dropdown, marking a stored model
    /// that is missing on the server (`plan/05 §3`). The editors are
    /// window-bound, so the update hops through the window handle.
    fn sync_model_select(&mut self, cx: &mut Context<Self>) {
        let models = self.settings_ui.models.clone();
        let selected = self.settings.model.clone();
        let mut items: Vec<SharedString> =
            models.iter().map(|model| model.clone().into()).collect();
        let mut index = models
            .iter()
            .position(|model| Some(model) == selected.as_ref());
        if let Some(model) = &selected
            && index.is_none()
        {
            items.push(format!("{model} (missing on server)").into());
            index = Some(items.len() - 1);
        }
        let Some(handle) = self.window else {
            return;
        };
        let _ = handle.update(cx, |_root, window, cx| {
            self.settings_ui.model.update(cx, |state, cx| {
                state.set_items(items, window, cx);
                state.set_selected_index(
                    index.map(|row| gpui_kit::component::IndexPath::default().row(row)),
                    window,
                    cx,
                );
            });
        });
    }

    /// Appends the user message + placeholder and starts the stream
    /// (`plan/01 §4`).
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().trim().to_string();
        if text.is_empty() || self.generation.is_some() {
            return;
        }
        let Some(model) = self.settings.model.clone() else {
            // `plan/03 §3`: no model selected → send blocked with a hint.
            self.store.update(cx, |store, _| {
                let chat_id = store.active_chat_id().expect("store starts with a chat");
                store.append_user_message(chat_id, &text);
                store.append_assistant_placeholder(chat_id, "-");
                store.fail_streaming(
                    chat_id,
                    "No model selected. Open Settings → Model.".to_owned(),
                );
            });
            cx.notify();
            return;
        };
        self.input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.input_empty = true;

        let chat_id = self.store.update(cx, |store, _| {
            let chat_id = store.active_chat_id().expect("store starts with a chat");
            store.append_user_message(chat_id, &text);
            store.append_assistant_placeholder(chat_id, &model);
            chat_id
        });
        self.persist_chat(chat_id, cx);

        // Compile the request from settings + the (cleaned) chat history.
        let history = self
            .store
            .read(cx)
            .chat(chat_id)
            .map(|chat| chat.messages.clone())
            .unwrap_or_default();
        let options = Options {
            temperature: self.settings.options.temperature,
            top_p: self.settings.options.top_p,
            top_k: self.settings.options.top_k,
            num_predict: self.settings.options.num_predict,
            num_ctx: self.settings.options.num_ctx,
            repeat_penalty: self.settings.options.repeat_penalty,
            seed: self.settings.options.seed,
        };
        let request =
            ollama::build_chat_request(&model, &self.settings.system_prompt, &options, &history);

        let handle = GenerationHandle {
            cancel: CancelFlag::new(),
            chat_id,
            started_at: chrono::Utc::now(),
        };
        self.generation = Some(handle.clone());
        cx.notify();
        self.scroll.scroll_to_bottom();

        self.spawn_generation(request, handle, cx);
    }

    /// Runs one generation: the blocking NDJSON loop on the background
    /// executor, chunks forwarded to the store on the main thread
    /// (`plan/04 §5`). A `std::sync::mpsc` bridge keeps the dependency set
    /// free of any async runtime; the foreground end polls with a short
    /// timer, which is invisible at chat cadence.
    fn spawn_generation(
        &mut self,
        request: ChatRequest,
        handle: GenerationHandle,
        cx: &mut Context<Self>,
    ) {
        let chat_id = handle.chat_id;
        let server_url = self.settings.server_url.clone();
        let model = self.settings.model.clone().unwrap_or_default();
        let client = match OllamaClient::new(&server_url) {
            Ok(client) => Arc::new(client),
            Err(error) => {
                let message = user_error_message(&error, &server_url, &model);
                self.store
                    .update(cx, |store, _| store.fail_streaming(chat_id, message));
                self.generation = None;
                cx.notify();
                return;
            }
        };

        let (tx, rx) = std::sync::mpsc::channel::<Result<ChatChunk, OllamaError>>();
        let cancel = handle.cancel.clone();

        // Blocking producer: `ureq` POST + line-by-line NDJSON reads, the
        // cancellation flag checked between lines (`plan/04 §6`).
        cx.background_executor()
            .spawn(async move {
                match client.chat(&request, &cancel) {
                    Ok(stream) => {
                        for item in stream {
                            if tx.send(item).is_err() {
                                break; // receiver dropped: app is quitting
                            }
                        }
                    }
                    Err(error) => {
                        tx.send(Err(error)).ok();
                    }
                }
            })
            .detach();

        // Foreground consumer: applies chunks to the ChatStore entity.
        cx.spawn(async move |this, cx| {
            loop {
                match rx.try_recv() {
                    Ok(Ok(chunk)) => {
                        let text = chunk.text().to_owned();
                        this.update(cx, |this, cx| {
                            if !text.is_empty() {
                                this.store
                                    .update(cx, |store, _| store.append_chunk(chat_id, &text));
                            }
                            cx.notify();
                        })
                        .ok();
                    }
                    Ok(Err(error)) => {
                        let stopped = error.is_cancelled();
                        let message = user_error_message(&error, &server_url, &model);
                        this.update(cx, |this, cx| {
                            this.store.update(cx, |store, _| {
                                if stopped {
                                    store.finish_streaming(chat_id, true);
                                } else {
                                    log::error!("chat stream failed: {error:?}");
                                    store.fail_streaming(chat_id, message);
                                }
                            });
                            this.persist_chat(chat_id, cx);
                            this.generation = None;
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    Err(TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(Duration::from_millis(15))
                            .await;
                    }
                    Err(TryRecvError::Disconnected) => {
                        // Producer finished without a done/error line: treat
                        // as a completed stream.
                        this.update(cx, |this, cx| {
                            this.store
                                .update(cx, |store, _| store.finish_streaming(chat_id, false));
                            this.persist_chat(chat_id, cx);
                            this.generation = None;
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                }
            }
        })
        .detach();
    }
}

/// Fetches `/api/tags` off the main thread.
async fn list_models_background(
    server_url: &str,
    cx: &mut AsyncApp,
) -> Result<Vec<String>, OllamaError> {
    let client = OllamaClient::new(server_url)?;
    cx.background_executor()
        .spawn(async move { client.list_models() })
        .await
}

/// First-run probe: the first installed model for auto-select (`plan/05 §6`).
async fn probe_first_model(this: &WeakEntity<Dislexia>, cx: &mut AsyncApp) -> Option<String> {
    let server_url = this
        .upgrade()?
        .read_with(cx, |this, _| this.settings.server_url.clone());
    let models = list_models_background(&server_url, cx).await.ok()?;
    let first = models.into_iter().next()?;
    this.update(cx, |this, cx| {
        this.settings_ui.models = Vec::new();
        this.refresh_models(cx);
    })
    .ok()?;
    Some(first)
}

impl Render for Dislexia {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.window.is_none() {
            self.window = Some(window.window_handle());
        }
        let theme = cx.theme();
        v_flex()
            .id("root")
            .on_action(cx.listener(|this, _: &NewChat, window, cx| {
                this.handle_new_chat(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Send, window, cx| this.handle_send(window, cx)))
            .on_action(
                cx.listener(|this, _: &StopGeneration, window, cx| this.handle_stop(window, cx)),
            )
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                settings_ui::open(this, window, cx);
            }))
            .on_action(cx.listener(|this, _: &HideWindow, window, _cx| {
                let _ = this; // the window closes; state lives on disk
                window.remove_window();
            }))
            .on_action(cx.listener(|_this, _: &QuitApp, _window, cx| {
                cx.quit();
            }))
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .child(
                            h_flex()
                                .px_2()
                                .gap_2()
                                .child("Dislexia")
                                .text_color(theme.muted_foreground)
                                .child(
                                    h_flex().font_family(theme.mono_font_family.clone()).child(
                                        div()
                                            .text_size(px(12.))
                                            .child(format!("· {}", self.model_label())),
                                    ),
                                ),
                        )
                        .child(
                            h_flex()
                                .ml_auto()
                                .gap_1()
                                .child(
                                    Button::new("title-new-chat")
                                        .small()
                                        .ghost()
                                        .icon(Icon::new(IconName::Plus))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.handle_new_chat(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("title-settings")
                                        .small()
                                        .ghost()
                                        .icon(Icon::new(IconName::Settings))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            settings_ui::open(this, window, cx);
                                        })),
                                ),
                        ),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(ui::sidebar::render(self, cx))
                    .child(ui::chat_pane::render(self, cx)),
            )
    }
}
