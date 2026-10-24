//! Settings dialog (`plan/03-ui-layout.md` §2, `plan/05-settings.md`).
//! One kit `Dialog`, sections Connection / Model / Parameters. Changes are
//! committed on every field change and persisted with a 300ms debounce;
//! there is no OK/Cancel.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogTitle;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::select::{Select, SelectState};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::Dislexia;

/// Width of the settings dialog (`plan/03 §2`).
const DIALOG_WIDTH: f32 = 560.;

/// Live connection state for the Test row.
#[derive(Clone, Debug)]
pub enum ConnectionStatus {
    Idle,
    Testing,
    Connected(usize),
    Failed(String),
}

pub struct SettingsUi {
    pub server_url: Entity<InputState>,
    pub system_prompt: Entity<TextareaState>,
    pub model: Entity<SelectState<Vec<SharedString>>>,
    pub temperature: Entity<InputState>,
    pub top_p: Entity<InputState>,
    pub top_k: Entity<InputState>,
    pub num_predict: Entity<InputState>,
    pub num_ctx: Entity<InputState>,
    pub repeat_penalty: Entity<InputState>,
    pub seed: Entity<InputState>,
    pub models: Vec<String>,
    pub status: ConnectionStatus,
    pub url_error: Option<String>,
    pub option_errors: Vec<&'static str>,
}

/// Creates every editor entity; called once from `Dislexia::with_settings`.
pub fn build_state(
    settings: &crate::settings::Settings,
    window: &mut Window,
    cx: &mut App,
) -> SettingsUi {
    fn input(
        window: &mut Window,
        placeholder: &'static str,
        value: String,
        cx: &mut App,
    ) -> Entity<InputState> {
        cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(value)
        })
    }

    let models = Vec::new();
    let model_items: Vec<SharedString> = Vec::new();
    let selected_index = None;
    let model =
        cx.new(|cx| SelectState::new(model_items, selected_index, window, cx).searchable(true));

    SettingsUi {
        server_url: input(
            window,
            "http://127.0.0.1:11434",
            settings.server_url.clone(),
            cx,
        ),
        system_prompt: cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(6, 12)
                .placeholder("No system prompt")
                .default_value(settings.system_prompt.clone())
        }),
        model,
        temperature: input(
            window,
            "0.0 – 2.0",
            option_value(settings.options.temperature),
            cx,
        ),
        top_p: input(
            window,
            "0.0 – 1.0",
            option_value(settings.options.top_p),
            cx,
        ),
        top_k: input(window, "≥ 0", option_value(settings.options.top_k), cx),
        num_predict: input(
            window,
            "tokens",
            option_value(settings.options.num_predict),
            cx,
        ),
        num_ctx: input(window, "tokens", option_value(settings.options.num_ctx), cx),
        repeat_penalty: input(
            window,
            "0.0 – 3.0",
            option_value(settings.options.repeat_penalty),
            cx,
        ),
        seed: input(
            window,
            "empty = random",
            option_value(settings.options.seed),
            cx,
        ),
        models,
        status: ConnectionStatus::Idle,
        url_error: None,
        option_errors: Vec::new(),
    }
}

/// The text an editor starts with: the stored value, or empty for "not sent".
fn option_value<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

/// Opens the settings dialog imperatively (gear button, `cmd-,`).
pub fn open(this: &mut Dislexia, window: &mut Window, cx: &mut Context<Dislexia>) {
    this.refresh_models(cx);
    this.test_connection(cx);

    let weak = cx.entity().downgrade();
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .w(px(DIALOG_WIDTH))
            .title(DialogTitle::new().child("Settings"))
            .content({
                let weak = weak.clone();
                move |content, _, cx| {
                    if let Some(app) = weak.upgrade() {
                        let this = app.read(cx);
                        content.child(render_content(this, weak.clone(), cx))
                    } else {
                        content
                    }
                }
            })
    });
}

fn render_content(this: &Dislexia, weak: WeakEntity<Dislexia>, cx: &App) -> Div {
    let theme = cx.theme();
    let ui = &this.settings_ui;

    let section_title = |text: &'static str| {
        div()
            .text_size(px(12.))
            .font_family(theme.mono_font_family.clone())
            .text_color(theme.muted_foreground)
            .child(text)
    };

    v_flex()
        .px_4()
        .pb_4()
        .gap_3()
        // Connection
        .child(section_title("CONNECTION"))
        .child(
            v_flex().gap_1().child(
                v_flex()
                    .gap_1()
                    .child(Input::new(&ui.server_url).font_family(theme.mono_font_family.clone()))
                    .children(ui.url_error.clone().map(|error| field_error(&error, cx))),
            ),
        )
        .child(render_test_row(this, weak.clone(), cx))
        .child(div().h_px().w_full().bg(theme.border))
        // Model
        .child(section_title("MODEL"))
        .child(Select::new(&ui.model).placeholder("Pick a model"))
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child("Run `ollama pull <name>` to add models"),
        )
        .child(div().h_px().w_full().bg(theme.border))
        // Parameters
        .child(section_title("PARAMETERS"))
        .child(
            v_flex().gap_1().child(
                v_flex()
                    .gap_1()
                    .child(Textarea::new(&ui.system_prompt))
                    .child(
                        h_flex().justify_end().child(
                            Button::new("reset-prompt")
                                .small()
                                .ghost()
                                .label("Reset to default")
                                .on_click({
                                    let weak = weak.clone();
                                    move |_, window, cx| {
                                        weak.update(cx, |this, cx| {
                                            this.reset_prompt(window, cx);
                                        })
                                        .ok();
                                    }
                                }),
                        ),
                    ),
            ),
        )
        .child(render_options_grid(this, cx))
}

/// Server URL row: status dot + Test button + result line.
fn render_test_row(this: &Dislexia, weak: WeakEntity<Dislexia>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let ui = &this.settings_ui;
    let (dot, label) = match &ui.status {
        ConnectionStatus::Idle => (theme.muted_foreground, "Not tested".to_owned()),
        ConnectionStatus::Testing => (theme.muted_foreground, "Testing…".to_owned()),
        ConnectionStatus::Connected(models) => {
            (theme.success, format!("Connected — {models} models"))
        }
        ConnectionStatus::Failed(error) => (theme.danger, error.clone()),
    };

    h_flex()
        .gap_2()
        .items_center()
        .child(div().size(px(8.)).rounded_full().bg(dot))
        .child(
            div()
                .flex_1()
                .text_size(px(13.))
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            Button::new("test-connection")
                .small()
                .secondary()
                .label("Test")
                .icon(Icon::new(IconName::RefreshCw))
                .on_click({
                    let weak = weak.clone();
                    move |_, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.test_connection(cx);
                        })
                        .ok();
                    }
                }),
        )
        .into_any_element()
}

/// The numeric parameter inputs, in a two-column grid (`plan/03 §2`).
fn render_options_grid(this: &Dislexia, cx: &App) -> Div {
    let ui = &this.settings_ui;
    let rows: [(&str, &Entity<InputState>, &'static str); 7] = [
        ("Temperature", &ui.temperature, "temperature"),
        ("Top P", &ui.top_p, "top_p"),
        ("Top K", &ui.top_k, "top_k"),
        ("Num predict", &ui.num_predict, "num_predict"),
        ("Num ctx", &ui.num_ctx, "num_ctx"),
        ("Repeat penalty", &ui.repeat_penalty, "repeat_penalty"),
        ("Seed", &ui.seed, "seed"),
    ];

    let mut grid = v_flex().gap_2();
    for pair in rows.chunks(2) {
        let mut row = h_flex().gap_2();
        for (label, state, field) in pair {
            row = row.child(
                v_flex()
                    .flex_1()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child((*label).to_string()),
                    )
                    .child(Input::new(state))
                    .when(ui.option_errors.contains(field), |cell| {
                        cell.child(field_error("out of range", cx))
                    }),
            );
        }
        grid = grid.child(row);
    }
    grid
}

fn field_error(message: &str, cx: &App) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().danger)
        .child(message.to_string())
        .into_any_element()
}
