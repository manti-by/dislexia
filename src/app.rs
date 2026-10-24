//! The `Dislexia` root entity: global state (settings, chat store, active
//! chat, streaming status), actions and wiring between UI and services.
//!
//! See `plan/01-architecture.md`. No HTTP and no file I/O here — blocking
//! work always goes through `cx.background_spawn` into the service modules.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::*;

use uuid::Uuid;

use crate::history::ChatStore;
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

/// Cancellation handle for the in-flight generation (`plan/04 §6`). Stored on
/// the root entity while a stream runs; the background loop checks `flag`
/// between lines.
#[derive(Clone)]
pub struct GenerationHandle {
    pub flag: Arc<AtomicBool>,
    pub chat_id: Uuid,
    #[allow(dead_code)] // read by the streaming meta line in M2
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// The root entity; every screen reads state from here.
pub struct Dislexia {
    store: Entity<ChatStore>,
    input: Entity<TextareaState>,
    /// Scroll handle of the transcript, for follow-on-append.
    scroll: ScrollHandle,
    /// The model label shown in the title bar; hardcoded until M3 settings.
    model: &'static str,
    /// Streaming generation, if any (one in-flight at a time, `plan/01 §1`).
    generation: Option<GenerationHandle>,
    /// True while the input holds only whitespace — Send is disabled then.
    input_empty: bool,
    _subscriptions: Vec<Subscription>,
}

impl Dislexia {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            model: "fake-model",
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

    pub fn model(&self) -> &str {
        self.model
    }

    /// The model label for the title bar; settings replace this in M3.
    pub fn model_label(&self) -> String {
        self.model.to_owned()
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
            handle.flag.store(true, Ordering::Relaxed);
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

    /// Appends the user message + placeholder and starts the (fake) stream.
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().trim().to_string();
        if text.is_empty() || self.generation.is_some() {
            return;
        }
        self.input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.input_empty = true;

        let chat_id = self.store.update(cx, |store, _| {
            let chat_id = store.active_chat_id().expect("store starts with a chat");
            store.append_user_message(chat_id, &text);
            store.append_assistant_placeholder(chat_id, self.model);
            chat_id
        });

        let handle = GenerationHandle {
            flag: Arc::new(AtomicBool::new(false)),
            chat_id,
            started_at: chrono::Utc::now(),
        };
        self.generation = Some(handle.clone());
        cx.notify();
        self.scroll.scroll_to_bottom();

        // M1 fake backend: a canned multi-chunk reply delivered like the real
        // stream will be in M2 — one append per chunk, flag checked between
        // chunks. M2 swaps this block for the ureq background loop.
        let flag = handle.flag.clone();
        let chunks = fake_reply(&text);
        cx.spawn(async move |this, cx| {
            for chunk in chunks {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                let delivered = this
                    .update(cx, |this, cx| {
                        this.store
                            .update(cx, |store, _| store.append_chunk(chat_id, &chunk));
                        cx.notify();
                    })
                    .is_ok();
                if !delivered {
                    return; // app is shutting down
                }
                cx.background_executor()
                    .timer(Duration::from_millis(45))
                    .await;
            }
            let stopped = flag.load(Ordering::Relaxed);
            this.update(cx, |this, cx| {
                this.store
                    .update(cx, |store, _| store.finish_streaming(chat_id, stopped));
                this.generation = None;
                cx.notify();
            })
            .ok();
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
                            h_flex()
                                .font_family(theme.mono_font_family.clone())
                                .child(div().text_size(px(12.)).child(format!("· {}", self.model))),
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

/// The M1 canned response, delivered in word-sized chunks to exercise the
/// incremental streaming render path.
fn fake_reply(user_text: &str) -> Vec<String> {
    let reply = format!(
        "(M1 echo — the real model arrives in M2.)\n\nYou sent: {user_text}\n\n\
         This fake stream exercises the incremental rendering path: chunks \
         arrive one by one, the cursor ▌ follows the text, and Stop cuts the \
         stream mid-sentence while keeping what has already been written."
    );
    reply
        .split_inclusive(' ')
        .map(str::to_owned)
        .collect::<Vec<_>>()
}
