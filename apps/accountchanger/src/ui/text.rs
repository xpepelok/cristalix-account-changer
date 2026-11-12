use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{
    App, Bounds, Div, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, ParentElement, Pixels, ShapedLine, SharedString, Style, Styled, Window, div, point,
    px,
};

thread_local! {
    static GLYPHS: RefCell<HashMap<char, SharedString>> = RefCell::new(HashMap::new());
}

fn glyph(ch: char) -> SharedString {
    GLYPHS.with_borrow_mut(|glyphs| {
        glyphs
            .entry(ch)
            .or_insert_with(|| SharedString::from(ch.to_string()))
            .clone()
    })
}

struct Spaced {
    chars: Vec<char>,
    spacing: Pixels,
}

impl IntoElement for Spaced {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Spaced {
    type RequestLayoutState = Vec<ShapedLine>;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Vec<ShapedLine>) {
        let text = window.text_style();
        let size = text.font_size.to_pixels(window.rem_size());
        let mut width = px(0.0);
        let lines: Vec<ShapedLine> = self
            .chars
            .iter()
            .map(|ch| {
                let glyph = glyph(*ch);
                let run = text.to_run(glyph.len());
                let line = window.text_system().shape_line(glyph, size, &[run], None);
                width += line.width + self.spacing;
                line
            })
            .collect();
        let mut style = Style::default();
        style.size.width = width.into();
        style.size.height = text.line_height_in_pixels(window.rem_size()).into();
        style.flex_shrink = 0.0;
        (window.request_layout(style, [], cx), lines)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _lines: &mut Vec<ShapedLine>,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        lines: &mut Vec<ShapedLine>,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let height = window.text_style().line_height_in_pixels(window.rem_size());
        let mut x = bounds.origin.x;
        for line in lines.iter() {
            line.paint(point(x, bounds.origin.y), height, window, cx)
                .ok();
            x += line.width + self.spacing;
        }
    }
}

pub fn spaced(text: &str, spacing: f32) -> Div {
    spaced_chars(text.chars(), spacing)
}

pub fn spaced_chars(chars: impl Iterator<Item = char>, spacing: f32) -> Div {
    div().flex().flex_none().whitespace_nowrap().child(Spaced {
        chars: chars.collect(),
        spacing: px(spacing),
    })
}

pub fn spaced_icons(
    chars: impl Iterator<Item = char>,
    spacing: f32,
    icon: f32,
    window: &Window,
    cx: &mut App,
) -> Div {
    let mut row = div().flex().flex_none().items_center().whitespace_nowrap();
    let mut run = Vec::new();
    let flush = |row: Div, run: &mut Vec<char>| {
        if run.is_empty() {
            return row;
        }
        row.child(Spaced {
            chars: std::mem::take(run),
            spacing: px(spacing),
        })
    };
    for ch in chars {
        if !ac_api::is_emoji(ch) {
            run.push(ch);
        } else if let Some(image) = crate::emoji::icon(ch, icon, window, cx) {
            row = flush(row, &mut run).child(div().mr(px(spacing)).child(image));
        }
    }
    flush(row, &mut run)
}
