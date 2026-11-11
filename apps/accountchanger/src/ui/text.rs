use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{Div, ParentElement, SharedString, Styled, div, px};

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

pub fn spaced(text: &str, spacing: f32) -> Div {
    spaced_chars(text.chars(), spacing)
}

pub fn spaced_chars(chars: impl Iterator<Item = char>, spacing: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .whitespace_nowrap()
        .children(chars.map(|ch| div().mr(px(spacing)).child(glyph(ch))))
}
