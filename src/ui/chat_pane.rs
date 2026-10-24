//! The chat pane: transcript scroll area + composer (input, Send/Stop)
//! (`plan/03-ui-layout.md` §1). All data comes from the `Dislexia` root.

use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Textarea;
use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::*;

use crate::app::Dislexia;
use crate::ui::message;

pub fn render(this: &mut Dislexia, cx: &mut Context<Dislexia>) -> impl IntoElement {
    let theme = cx.theme();
    let chat = this.store().read(cx).active_chat().cloned();
    let streaming = this.is_streaming();
    let send_disabled = this.input_empty() || streaming;
    let input = this.input().clone();

    let mut transcript = v_flex()
        .id("transcript")
        .flex_1()
        .min_h_0()
        .track_scroll(this.scroll_handle())
        .overflow_y_scroll()
        .px_4()
        .py_3()
        .gap_3()
        .max_w(px(860.))
        .mx_auto();

    match chat {
        Some(chat) if !chat.messages.is_empty() => {
            for (index, message) in chat.messages.iter().enumerate() {
                transcript =
                    transcript.child(message::render(format!("msg-{}", index), message, cx));
            }
        }
        _ => {
            transcript = transcript.child(
                h_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child("Start a conversation with a local model ↓"),
            );
        }
    }

    v_flex()
        .flex_1()
        .min_h_0()
        .bg(theme.background)
        // Transcript
        .child(div().flex_1().min_h_0().overflow_hidden().child(transcript))
        // Composer
        .child(
            h_flex()
                .flex_shrink_0()
                .border_t_1()
                .border_color(theme.border)
                .bg(theme.background)
                .px_4()
                .py_3()
                .gap_2()
                .items_end()
                .child(Textarea::new(&input))
                .child(if streaming {
                    Button::new("stop")
                        .danger()
                        .label("Stop")
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.handle_stop(cx);
                        }))
                } else {
                    Button::new("send")
                        .primary()
                        .label("Send")
                        .disabled(send_disabled)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.handle_send(window, cx);
                        }))
                }),
        )
}
