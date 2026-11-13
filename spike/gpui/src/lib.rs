use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, div, rgb,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};

pub struct Counter {
    pub count: i64,
    pub name: String,
    input: Entity<InputState>,
    _changes: Subscription,
}

impl Counter {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let changes = cx.subscribe(&input, |this: &mut Self, input, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.name = input.read(cx).value().to_string();
                cx.notify();
            }
        });
        Self {
            count: 0,
            name: String::new(),
            input,
            _changes: changes,
        }
    }
}

impl Render for Counter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x000000))
            .child(format!("Count {}", self.count))
            .child(format!("Name {}", self.name))
            .child(
                div().debug_selector(|| "add".to_string()).child(
                    Button::new("add")
                        .label("Add")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.count += 1;
                            cx.notify();
                        })),
                ),
            )
            .child(
                div()
                    .debug_selector(|| "name".to_string())
                    .child(Input::new(&self.input)),
            )
    }
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    gpui_platform::single_threaded_web().run(|cx: &mut gpui::App| {
        let fonts = vec![
            std::borrow::Cow::Borrowed(include_bytes!("../fonts/IBMPlexSans-Regular.ttf").as_slice()),
            std::borrow::Cow::Borrowed(include_bytes!("../fonts/IBMPlexSans-SemiBold.ttf").as_slice()),
        ];
        if let Err(error) = cx.text_system().add_fonts(fonts) {
            web_sys_free_log(&error.to_string());
        }
        gpui_component::init(cx);
        let opened = cx.open_window(gpui::WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| Counter::new(window, cx));
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        });
        drop(opened);
    });
}

#[cfg(target_family = "wasm")]
fn web_sys_free_log(_text: &str) {}
