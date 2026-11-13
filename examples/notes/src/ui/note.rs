use gpui::{
    Context, EventEmitter, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div,
};
use gpui_component::button::Button;

pub enum Move {
    Back,
    Saved(String, String),
}

pub struct Note {
    pub title: String,
    pub text: String,
    pub left: bool,
}

impl EventEmitter<Move> for Note {}

impl Note {
    #[must_use]
    pub fn make(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let note = Self {
            title: String::new(),
            text: String::new(),
            left: false,
        };
        debug_assert!(note.title.is_empty());
        debug_assert!(!note.left);
        note
    }

    pub fn show(&mut self, title: String, text: String, cx: &mut Context<Self>) {
        self.title = title;
        self.text = text;
        self.left = false;
        debug_assert!(!self.left);
        debug_assert!(self.title.len() < 1 << 16);
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        self.left = true;
        cx.emit(Move::Back);
        debug_assert!(self.left);
        debug_assert!(self.text.len() < 1 << 16);
        cx.notify();
    }
}

impl Render for Note {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        debug_assert!(self.title.len() < 1 << 16);
        debug_assert!(self.text.len() < 1 << 16);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child(self.title.clone()))
            .child(self.text.clone())
            .child(
                div().debug_selector(|| "back".to_string()).child(
                    Button::new("back")
                        .label("Back")
                        .on_click(cx.listener(|note, _, _, cx| note.back(cx))),
                ),
            )
    }
}
