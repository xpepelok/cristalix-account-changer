use gpui::{FontWeight, Pixels, SharedString, TextRun, Window, px};

use crate::theme::FONT_FAMILY;

pub fn text_width(text: &str, size: f32, weight: FontWeight, spacing: f32, window: &Window) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let run = TextRun {
        len: text.len(),
        font: gpui::Font {
            weight,
            ..gpui::font(FONT_FAMILY)
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = window.text_system().shape_line(
        SharedString::from(text.to_owned()),
        px(size),
        &[run],
        None,
    );
    f32::from(shaped.width) + spacing * text.chars().count() as f32
}

pub fn fit_size(
    text: &str,
    base: f32,
    min: f32,
    weight: FontWeight,
    budget: f32,
    window: &Window,
) -> Pixels {
    let width = text_width(text, base, weight, 0.0, window);
    if width <= budget || width <= 0.0 {
        return px(base);
    }
    px((base * budget / width).floor().max(min))
}

pub fn fits(text: &str, size: f32, weight: FontWeight, budget: f32, window: &Window) -> bool {
    text_width(text, size, weight, 0.0, window) <= budget
}

pub fn chip_width(label: &str, compact: bool, window: &Window) -> f32 {
    let (size, pad) = if compact { (8.8, 10.0) } else { (9.6, 14.0) };
    text_width(
        &label.to_uppercase(),
        size,
        FontWeight::EXTRA_BOLD,
        0.4,
        window,
    ) + pad
}
