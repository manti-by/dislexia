//! The chat pane: transcript scroll area + composer (input, Send/Stop)
//! (`plan/03-ui-layout.md` §1). All data comes from the `Dislexia` root.
//!
//! The composer sits at the window bottom as a floating rounded box, with
//! the Send button inside its bottom-right corner (zcode-like).

use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Textarea;
use gpui_kit::component::{ActiveTheme, Sizable as _, h_flex, v_flex};
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
        .pt_3()
        .pb_1()
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

    let action_button = if streaming {
        Button::new("stop")
            .small()
            .danger()
            .label("Stop")
            .on_click(cx.listener(|this, _, _window, cx| {
                this.handle_stop(cx);
            }))
    } else {
        Button::new("send")
            .small()
            .primary()
            .label("Send")
            .disabled(send_disabled)
            .on_click(cx.listener(|this, _, window, cx| {
                this.handle_send(window, cx);
            }))
    };

    v_flex()
        .flex_1()
        .min_h_0()
        .bg(theme.background)
        // Transcript
        .child(div().flex_1().min_h_0().overflow_hidden().child(transcript))
        // Composer: floating rounded box at the window bottom, button inside
        // its bottom-right corner.
        .child(
            h_flex().flex_shrink_0().px_4().pb_3().pt_1().child(
                div()
                    .relative()
                    .flex_1()
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.input_background())
                    .shadow_sm()
                    .child(
                        Textarea::new(&input)
                            .appearance(false)
                            .bordered(false)
                            .pr_16()
                            .pb_8(),
                    )
                    .child(div().absolute().bottom_2().right_2().child(action_button)),
            ),
        )
}
