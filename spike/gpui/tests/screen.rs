use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use gpui::{
    AppContext, Bounds, Context, DevicePixels, Entity, Font, FontId, FontMetrics, FontRun, GlyphId, Styled,
    IntoElement, LineLayout, Modifiers, NoopTextSystem, ParentElement, Pixels, PlatformTextSystem,
    Render, RenderGlyphParams, Size, TestAppContext, TestDispatcher, TextRenderingMode, Window,
    div,
};
use spike::Counter;

struct Shown {
    noop: NoopTextSystem,
    lines: Mutex<Vec<String>>,
}

impl PlatformTextSystem for Shown {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> gpui::Result<()> {
        self.noop.add_fonts(fonts)
    }
    fn all_font_names(&self) -> Vec<String> {
        self.noop.all_font_names()
    }
    fn font_id(&self, descriptor: &Font) -> gpui::Result<FontId> {
        self.noop.font_id(descriptor)
    }
    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        self.noop.font_metrics(font_id)
    }
    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> gpui::Result<Bounds<f32>> {
        self.noop.typographic_bounds(font_id, glyph_id)
    }
    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> gpui::Result<Size<f32>> {
        self.noop.advance(font_id, glyph_id)
    }
    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.noop.glyph_for_char(font_id, ch)
    }
    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> gpui::Result<Bounds<DevicePixels>> {
        self.noop.glyph_raster_bounds(params)
    }
    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> gpui::Result<(Size<DevicePixels>, Vec<u8>)> {
        self.noop.rasterize_glyph(params, raster_bounds)
    }
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        if let Ok(mut lines) = self.lines.lock() {
            lines.push(text.to_string());
        }
        self.noop.layout_line(text, font_size, runs)
    }
    fn recommended_rendering_mode(&self, font_id: FontId, font_size: Pixels) -> TextRenderingMode {
        self.noop.recommended_rendering_mode(font_id, font_size)
    }
}

struct Probe {
    screen: Entity<Counter>,
    visible: bool,
}

impl Render for Probe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.visible {
            div().size_full().child(self.screen.clone())
        } else {
            div().size_full()
        }
    }
}

fn shown_now(cx: &mut gpui::VisualTestContext, probe: &Entity<Probe>, shown: &Shown) -> String {
    probe.update(cx, |p, cx| {
        p.visible = false;
        cx.notify();
    });
    cx.run_until_parked();
    if let Ok(mut lines) = shown.lines.lock() {
        lines.clear();
    }
    probe.update(cx, |p, cx| {
        p.visible = true;
        cx.notify();
    });
    cx.run_until_parked();
    shown.lines.lock().map(|l| l.join("\n")).unwrap_or_default()
}

#[test]
fn a_click_and_typing_change_the_state_and_the_text() {
    let shown = Arc::new(Shown {
        noop: NoopTextSystem,
        lines: Mutex::new(Vec::new()),
    });
    let mut app = TestAppContext::build_with_text_system(TestDispatcher::new(0), None, shown.clone());
    app.update(gpui_component::init);
    let mut slot: Option<(Entity<Counter>, Entity<Probe>)> = None;
    let window = app.add_window(|window, cx| {
        let screen = cx.new(|cx| Counter::new(window, cx));
        let view = cx.new(|_| Probe {
            screen: screen.clone(),
            visible: true,
        });
        slot = Some((screen, view.clone()));
        gpui_component::Root::new(view, window, cx)
    });
    let (screen, probe) = slot.expect("the screen");
    let cx = &mut gpui::VisualTestContext::from_window(window.into(), &app);
    cx.run_until_parked();
    screen.update(cx, |s, cx| {
        s.count = 41;
        cx.notify();
    });
    cx.run_until_parked();
    let add = cx.debug_bounds("add").expect("the bounds of add");
    cx.simulate_click(add.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(screen.read_with(cx, |s, _| s.count), 42);
    let name = cx.debug_bounds("name").expect("the bounds of name");
    cx.simulate_click(name.center(), Modifiers::none());
    cx.simulate_input("Ada");
    cx.run_until_parked();
    assert_eq!(screen.read_with(cx, |s, _| s.name.clone()), "Ada");
    let all = shown.lines.lock().map(|l| l.join("\n")).unwrap_or_default();
    println!("all text laid out so far:\n{all}\n----");
    let text = shown_now(cx, &probe, &shown);
    println!("the text of the last frame:\n{text}\n----");
    assert!(text.contains("Count 42"), "{text}");
    assert!(text.contains("Name Ada"), "{text}");
    assert!(!text.contains("Count 41"), "{text}");
}
