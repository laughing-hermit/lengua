use gpui::{
    AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Subscription, Window, div, rgb,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use http_client::{AsyncBody, HttpClient};

use crate::note::Move;

pub const TITLE_LIMIT: usize = 40;

pub struct Edit {
    pub title: String,
    pub text: String,
    pub error: String,
    pub sent: bool,
    pub left: bool,
    title_input: Entity<InputState>,
    text_input: Entity<InputState>,
    changes: Vec<Subscription>,
}

impl EventEmitter<Move> for Edit {}

#[must_use]
pub fn check(title: &str) -> Option<&'static str> {
    let found = if title.trim().is_empty() {
        Some("The title is empty.")
    } else if title.chars().count() > TITLE_LIMIT {
        Some("The title is too long.")
    } else {
        None
    };
    debug_assert!(found.is_some() || !title.trim().is_empty());
    debug_assert!(found.is_some() || title.chars().count() <= TITLE_LIMIT);
    found
}

impl Edit {
    #[must_use]
    pub fn make(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("Title"));
        let text_input = cx.new(|cx| InputState::new(window, cx).placeholder("Text"));
        let changes = vec![
            cx.subscribe(&title_input, |edit: &mut Self, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    edit.title = input.read(cx).value().to_string();
                    cx.notify();
                }
            }),
            cx.subscribe(&text_input, |edit: &mut Self, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    edit.text = input.read(cx).value().to_string();
                    cx.notify();
                }
            }),
        ];
        debug_assert_eq!(changes.len(), 2);
        debug_assert!(title_input.entity_id() != text_input.entity_id());
        Self {
            title: String::new(),
            text: String::new(),
            error: String::new(),
            sent: false,
            left: false,
            title_input,
            text_input,
            changes,
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.error = check(&self.title).unwrap_or_default().to_string();
        if self.error.is_empty() {
            self.sent = true;
            let body = json::write(&json::Value::Object(vec![
                ("title".to_string(), json::Value::Text(self.title.clone())),
                ("text".to_string(), json::Value::Text(self.text.clone())),
            ]));
            let client = cx.http_client();
            cx.background_spawn(async move {
                let _ = client.post_json("/api/notes", AsyncBody::from(body)).await;
            })
            .detach();
            cx.emit(Move::Saved(self.title.clone(), self.text.clone()));
        }
        debug_assert!(self.sent || !self.error.is_empty());
        debug_assert!(!self.sent || self.title.chars().count() <= TITLE_LIMIT);
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.left = true;
        cx.emit(Move::Back);
        debug_assert!(self.left);
        debug_assert!(self.changes.len() == 2);
        cx.notify();
    }
}

impl Render for Edit {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut form = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xl().child("New note"))
            .child("Title")
            .child(
                div()
                    .debug_selector(|| "title".to_string())
                    .child(Input::new(&self.title_input)),
            )
            .child("Text")
            .child(
                div()
                    .debug_selector(|| "text".to_string())
                    .child(Input::new(&self.text_input)),
            );
        if !self.error.is_empty() {
            form = form.child(div().text_color(rgb(0x00b0_0020)).child(self.error.clone()));
        }
        debug_assert!(self.error.len() < 64);
        debug_assert!(self.title.len() < 1 << 16);
        form.child(
            div()
                .flex()
                .gap_2()
                .child(
                    div().debug_selector(|| "save".to_string()).child(
                        Button::new("save")
                            .label("Save")
                            .on_click(cx.listener(|edit, _, _, cx| edit.save(cx))),
                    ),
                )
                .child(
                    div().debug_selector(|| "cancel".to_string()).child(
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(cx.listener(|edit, _, _, cx| edit.cancel(cx))),
                    ),
                ),
        )
    }
}
