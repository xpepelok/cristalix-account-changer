use gpui::{Div, ParentElement, Styled, div, px};

pub fn spaced(text: &str, spacing: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .whitespace_nowrap()
        .children(text.chars().map(|ch| {
            let mut buffer = [0u8; 4];
            div().mr(px(spacing)).child(gpui::SharedString::from(
                ch.encode_utf8(&mut buffer).to_owned(),
            ))
        }))
}
