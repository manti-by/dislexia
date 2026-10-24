//! One message row (`plan/03-ui-layout.md` §1 "Message row anatomy").
//! User: surface panel, right-aligned. Assistant: flat block with a 2px
//! accent bar, markdown body, monospace meta line; a streaming cursor runs
//! while the message streams; errors render with a red bar.

use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::*;

use crate::app::Dislexia;
use crate::history::{Message, MessageStatus, Role};

const CURSOR: &str = "▌";

pub fn render(id: impl Into<ElementId>, message: &Message, cx: &Context<Dislexia>) -> AnyElement {
    match message.role {
        Role::User => render_user(message, cx).into_any_element(),
        Role::Assistant => render_assistant(id, message, cx).into_any_element(),
    }
}

fn render_user(message: &Message, cx: &Context<Dislexia>) -> impl IntoElement {
    let theme = cx.theme();

    h_flex().w_full().justify_end().child(
        v_flex()
            .max_w(px(520.))
            .rounded(theme.radius)
            .bg(theme.sidebar)
            .border_1()
            .border_color(theme.border)
            .px_3()
            .py_2()
            .child(meta_line("you", None, cx))
            .child(div().mt_1().child(message.content.clone())),
    )
}

fn render_assistant(
    id: impl Into<ElementId>,
    message: &Message,
    cx: &Context<Dislexia>,
) -> impl IntoElement {
    let theme = cx.theme();
    let id: ElementId = id.into();
    let streaming = message.status.is_streaming();

    let (bar_color, body) = match &message.status {
        MessageStatus::Error { message } => (
            theme.danger,
            div()
                .text_color(theme.danger)
                .child(message.clone())
                .into_any_element(),
        ),
        _status => (
            theme.primary,
            div()
                .text_color(theme.foreground)
                .child(TextView::markdown(
                    id,
                    format!("{}{}", message.content, cursor(streaming)),
                ))
                .into_any_element(),
        ),
    };

    let model = status_model(&message.status);

    v_flex()
        .w_full()
        .border_l_2()
        .border_color(bar_color)
        .pl_3()
        .py_0p5()
        .gap_1()
        .child(meta_line("assistant", model, cx))
        .child(body)
}

/// The streaming cursor: only while the message is live.
fn cursor(streaming: bool) -> String {
    if streaming {
        CURSOR.to_owned()
    } else {
        String::new()
    }
}

fn status_model(status: &MessageStatus) -> Option<String> {
    match status {
        MessageStatus::Streaming { model, .. } => Some(model.clone()),
        _ => None,
    }
}

/// The dim monospace metadata line above a message body.
fn meta_line(author: &str, detail: Option<String>, cx: &Context<Dislexia>) -> impl IntoElement {
    let theme = cx.theme();
    let text = match detail {
        Some(detail) => format!("{author} · {detail}"),
        None => author.to_string(),
    };
    div()
        .text_size(px(11.))
        .font_family(theme.mono_font_family.clone())
        .text_color(theme.muted_foreground)
        .child(text)
}
