//! The sidebar: New chat button + the list of previous chats
//! (`plan/03-ui-layout.md` §1). Renders from the `ChatStore` entity; the
//! only interaction is selecting a chat (delete/rename arrive in M4).

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::Dislexia;

/// Width of the sidebar column.
pub const WIDTH: f32 = 240.;

pub fn render(this: &mut Dislexia, cx: &mut Context<Dislexia>) -> impl IntoElement {
    let theme = cx.theme();
    let chats = this.store().read(cx).sorted_chats();
    let active_chat_id = this.store().read(cx).active_chat_id();

    let mut list = v_flex()
        .id("chat-list")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .gap_px();

    for chat in chats {
        let id = chat.id;
        let title = if chat.title.is_empty() {
            "New chat".to_string()
        } else {
            chat.title.clone()
        };
        let is_active = Some(id) == active_chat_id;

        list = list.child(
            h_flex()
                .id(format!("chat-{id}"))
                .px_2()
                .py_1p5()
                .rounded(theme.radius)
                .text_size(px(13.))
                .text_color(if is_active {
                    theme.sidebar_accent_foreground
                } else {
                    theme.sidebar_foreground
                })
                .when(is_active, |row| {
                    row.bg(theme.sidebar_accent)
                        .border_l_3()
                        .border_color(theme.primary)
                })
                .hover(|row| row.bg(theme.sidebar_accent))
                .truncate()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.store().update(cx, |store, _| store.set_active(id));
                    cx.notify();
                    this.focus_input(window, cx);
                }))
                .child(title),
        );
    }

    v_flex()
        .w(px(WIDTH))
        .h_full()
        .flex_shrink_0()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.sidebar_border)
        .p_2()
        .gap_2()
        .child(
            Button::new("new-chat")
                .small()
                .secondary()
                .label("New chat")
                .icon(Icon::new(IconName::Plus))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.handle_new_chat(window, cx);
                })),
        )
        .child(list)
}
