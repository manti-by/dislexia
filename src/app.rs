//! The `Dislexia` root entity: global state (settings, chat store, active
//! chat, streaming status), actions and wiring between UI and services.
//!
//! See `plan/01-architecture.md`. No HTTP and no file I/O here — blocking
//! work always goes through the background executor into the service
//! modules.

use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::*;

use uuid::Uuid;

use crate::history::ChatStore;
use crate::ollama::{
    self, CancelFlag, ChatChunk, ChatRequest, OllamaClient, OllamaError, Options,
    user_error_message,
};
use crate::settings::Settings;
use crate::ui;

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

/// The root entity; every screen reads state from here.
pub struct Dislexia {
    store: Entity<ChatStore>,
    input: Entity<TextareaState>,
    /// Scroll handle of the transcript, for follow-on-append.
    scroll: ScrollHandle,
    /// Chat settings (`plan/05`); persisted + editable in M3.
    settings: Settings,
    /// Streaming generation, if any (one in-flight at a time, `plan/01 §1`).
    generation: Option<GenerationHandle>,
    /// True while the input holds only whitespace — Send is disabled then.
    input_empty: bool,
    _subscriptions: Vec<Subscription>,
}

impl Dislexia {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // M2: hardcoded settings only — no file I/O yet (M3 wires load/save
        // and the settings UI). The model here is a local stand-in until the
        // first-run auto-select (`plan/05 §6`) replaces it in M3.
        let settings = Settings {
            model: Some("llama3:8b".to_owned()),
            ..Settings::default()
        };
        Self::with_settings(settings, window, cx)
    }

    pub fn with_settings(settings: Settings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.new(|_| ChatStore::new());
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 6)
                .submit_on_enter(true)
                .placeholder("Paste text to fix, or ask anything…")
        });

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

        Self {
            store,
            input,
            scroll: ScrollHandle::new(),
            settings,
            generation: None,
            input_empty: true,
            _subscriptions: subscriptions,
        }
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
            Ok(client) => std::sync::Arc::new(client),
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

impl Render for Dislexia {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                TitleBar::new().child(
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
