use futures::AsyncReadExt;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Window, WindowOptions, div, rgb,
};
use gpui_component::Root;
use gpui_component::button::Button;
use http_client::{AsyncBody, HttpClient};
use json::Value;

use crate::edit::Edit;
use crate::note::{Move, Note};

pub struct List {
    pub data: String,
    pub page: String,
    pub chosen: i64,
    pub status: String,
    note: Entity<Note>,
    edit: Entity<Edit>,
    moves: Vec<Subscription>,
}

pub fn start(cx: &mut App) {
    gpui_component::init(cx);
    let opened = cx.open_window(WindowOptions::default(), |window, cx| {
        let list = cx.new(|cx| List::make(window, cx));
        list.update(cx, |list, cx| list.load(cx));
        cx.new(|cx| Root::new(list, window, cx))
    });
    debug_assert!(opened.is_ok());
    debug_assert!(!cx.windows().is_empty());
}

fn follow<T: EventEmitter<Move> + 'static>(
    cx: &mut Context<List>,
    entity: &Entity<T>,
) -> Subscription {
    debug_assert!(entity.entity_id().as_u64() > 0);
    debug_assert!(cx.entity_id().as_u64() > 0);
    cx.subscribe(entity, |list: &mut List, _, event: &Move, cx| {
        list.moved(event, cx);
    })
}

fn field(item: &Value, name: &str) -> String {
    let text = if let Some(Value::Text(text)) = json::get(item, name) {
        text.clone()
    } else {
        String::new()
    };
    debug_assert!(!name.is_empty());
    debug_assert!(text.len() < 1 << 16);
    text
}

impl List {
    #[must_use]
    pub fn make(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let note = cx.new(|cx| Note::make(window, cx));
        let edit = cx.new(|cx| Edit::make(window, cx));
        let moves = vec![follow(cx, &note), follow(cx, &edit)];
        debug_assert_eq!(moves.len(), 2);
        debug_assert!(note.entity_id() != edit.entity_id());
        Self {
            data: "[]".to_string(),
            page: "list".to_string(),
            chosen: -1,
            status: "ready".to_string(),
            note,
            edit,
            moves,
        }
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        self.status = "loading".to_string();
        let client = cx.http_client();
        cx.spawn(async move |list, cx| {
            let mut text = String::new();
            let answer = client.get("/api/notes", AsyncBody::empty(), true).await;
            let read = if let Ok(mut answer) = answer
                && answer.status().is_success()
            {
                answer.body_mut().read_to_string(&mut text).await.is_ok()
            } else {
                false
            };
            let _ = list.update(cx, |list, cx| list.loaded(read, text, cx));
        })
        .detach();
        debug_assert_eq!(self.status, "loading");
        debug_assert!(self.chosen >= -1);
    }

    fn loaded(&mut self, read: bool, text: String, cx: &mut Context<Self>) {
        if read && matches!(json::parse(&text), Ok(Value::List(_))) {
            self.data = text;
            self.status = "ready".to_string();
        } else {
            self.status = "failed".to_string();
        }
        debug_assert!(!self.status.is_empty());
        debug_assert!(!self.data.is_empty());
        cx.notify();
    }

    fn notes(&self) -> Vec<(String, String)> {
        let Ok(Value::List(items)) = json::parse(&self.data) else {
            return Vec::new();
        };
        let found: Vec<(String, String)> = items
            .iter()
            .map(|item| (field(item, "title"), field(item, "text")))
            .collect();
        debug_assert_eq!(found.len(), items.len());
        debug_assert!(found.len() < 1 << 16);
        found
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        let notes = self.notes();
        let Some((title, text)) = notes.get(index).cloned() else {
            return;
        };
        self.note.update(cx, |note, cx| note.show(title, text, cx));
        self.chosen = i64::try_from(index).unwrap_or(-1);
        self.page = "note".to_string();
        debug_assert!(self.chosen >= 0);
        debug_assert!(index < notes.len());
        cx.notify();
    }

    fn new_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let edit = cx.new(|cx| Edit::make(window, cx));
        self.moves.truncate(1);
        self.moves.push(follow(cx, &edit));
        self.edit = edit;
        self.page = "edit".to_string();
        debug_assert_eq!(self.moves.len(), 2);
        debug_assert_eq!(self.page, "edit");
        cx.notify();
    }

    fn moved(&mut self, event: &Move, cx: &mut Context<Self>) {
        if let Move::Saved(title, text) = event {
            self.add(title, text);
        }
        self.page = "list".to_string();
        debug_assert_eq!(self.page, "list");
        debug_assert!(!self.data.is_empty());
        cx.notify();
    }

    fn add(&mut self, title: &str, text: &str) {
        let mut items = if let Ok(Value::List(items)) = json::parse(&self.data) {
            items
        } else {
            Vec::new()
        };
        let id = (items.len() + 1).to_string();
        items.push(Value::Object(vec![
            ("id".to_string(), Value::Text(id)),
            ("title".to_string(), Value::Text(title.to_string())),
            ("text".to_string(), Value::Text(text.to_string())),
        ]));
        self.data = json::write(&Value::List(items));
        debug_assert!(self.data.starts_with('['));
        debug_assert!(self.data.contains("title"));
    }

    fn rows(&self, cx: &mut Context<Self>) -> gpui::Div {
        let notes = self.notes();
        let mut column = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xl().child("Notes"));
        if notes.is_empty() {
            column = column.child("There are no notes.");
        }
        if self.status == "failed" {
            column = column.child("The notes did not load.");
        }
        for (index, (title, _)) in notes.iter().enumerate() {
            let selector = format!("note-{index}");
            column = column.child(
                div().debug_selector(move || selector).child(
                    Button::new(SharedString::from(format!("note-{index}")))
                        .label(title.clone())
                        .on_click(cx.listener(move |list, _, _, cx| list.choose(index, cx))),
                ),
            );
        }
        debug_assert!(notes.len() < 1 << 16);
        debug_assert!(!self.status.is_empty());
        column.child(
            div().debug_selector(|| "new".to_string()).child(
                Button::new("new")
                    .label("New note")
                    .on_click(cx.listener(|list, _, window, cx| list.new_note(window, cx))),
            ),
        )
    }
}

impl Render for List {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.page.as_str() {
            "note" => div().child(self.note.clone()),
            "edit" => div().child(self.edit.clone()),
            _ => self.rows(cx),
        };
        debug_assert!(!self.page.is_empty());
        debug_assert!(self.chosen >= -1);
        div()
            .size_full()
            .bg(rgb(0x00ff_ffff))
            .text_color(rgb(0x0020_2020))
            .p_4()
            .child(body)
    }
}
