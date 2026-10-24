//! The sidebar: New chat button + the grouped list of previous chats
//! (`plan/03-ui-layout.md` §1, `plan/06 §4`). Rows select on click and
//! reveal a delete action on hover (confirmed via an alert dialog).

use chrono::Utc;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::Dislexia;
use crate::history::group_label;

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

    let mut last_group: Option<&'static str> = None;
    for chat in chats {
        // Group headers: Today / Yesterday / Previous 7 days / Older
        // (`plan/06 §4`).
        let group = group_label(chat.updated_at, Utc::now());
        if last_group != Some(group) {
            list = list.child(
                div()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .text_size(px(11.))
                    .font_family(theme.mono_font_family.clone())
                    .text_color(theme.muted_foreground)
                    .child(group),
            );
            last_group = Some(group);
        }

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
                .gap_1()
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
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.store().update(cx, |store, _| store.set_active(id));
                    cx.notify();
                    this.focus_input(window, cx);
                }))
                // Title truncates against the delete button.
                .child(div().flex_1().min_w_0().truncate().child(title))
                .child(
                    Button::new(format!("delete-chat-{id}"))
                        .with_size(gpui_kit::component::Size::XSmall)
                        .ghost()
                        .icon(Icon::new(IconName::Delete))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            confirm_delete(this, id, window, cx);
                        })),
                ),
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

/// Delete-with-confirm (`plan/03 §4`, v1 minimum).
fn confirm_delete(
    this: &mut Dislexia,
    chat_id: uuid::Uuid,
    window: &mut Window,
    cx: &mut Context<Dislexia>,
) {
    let title = this
        .store()
        .read(cx)
        .chat(chat_id)
        .map(|chat| chat.title.clone())
        .unwrap_or_default();
    let description = if title.is_empty() {
        "Delete this chat? This cannot be undone.".to_owned()
    } else {
        format!("Delete \"{title}\"? This cannot be undone.")
    };
    let weak = cx.entity().downgrade();
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title("Delete chat")
            .description(description.clone())
            .ok_text("Delete")
            .ok_variant(gpui_kit::component::button::ButtonVariant::Danger)
            .show_cancel(true)
            .on_ok({
                let weak = weak.clone();
                move |_, _, cx| {
                    weak.update(cx, |this, cx| this.handle_delete_chat(chat_id, cx))
                        .is_ok()
                }
            })
    });
}
