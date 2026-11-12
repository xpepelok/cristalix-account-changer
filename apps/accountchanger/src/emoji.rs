use std::collections::HashMap;
use std::sync::Arc;

use ac_api::{EmojiSet, Rank, is_emoji};
use gpui::{
    AnyElement, App, Div, Global, IntoElement, ParentElement, RenderImage, SharedString, Styled,
    Window, div, img, px,
};
use image::{RgbaImage, imageops};

use crate::skin::to_render_image;

const SOURCE_MAX: u32 = 64;

#[derive(Default)]
pub struct EmojiImages {
    sources: HashMap<u32, RgbaImage>,
    sized: HashMap<(u32, u32), Arc<RenderImage>>,
}

impl Global for EmojiImages {}

pub fn install(images: Vec<(u32, RgbaImage)>, cx: &mut App) {
    let store = cx.default_global::<EmojiImages>();
    for (code, image) in images {
        store.sized.retain(|(sized, _), _| *sized != code);
        store.sources.insert(code, image);
    }
}

fn blank(image: &RgbaImage) -> bool {
    image.pixels().all(|pixel| pixel.0[3] == 0)
}

fn sized(source: &RgbaImage, side: u32) -> RgbaImage {
    let (width, height) = source.dimensions();
    let longest = width.max(height).max(1);
    if longest == side {
        return source.clone();
    }
    let step = if longest < side {
        side.div_ceil(longest)
    } else {
        1
    };
    let (width, height, longest) = (width * step, height * step, longest * step);
    let sharp = imageops::resize(source, width, height, imageops::FilterType::Nearest);
    if longest == side {
        return sharp;
    }
    let scaled = |value: u32| (value * side / longest).max(1);
    let mut sharp = sharp;
    weigh(&mut sharp, |channel, alpha| channel * alpha / 255);
    let mut smooth = imageops::resize(
        &sharp,
        scaled(width),
        scaled(height),
        imageops::FilterType::Triangle,
    );
    weigh(&mut smooth, |channel, alpha| {
        (channel * 255).checked_div(alpha).unwrap_or(0).min(255)
    });
    smooth
}

fn weigh(image: &mut RgbaImage, apply: impl Fn(u32, u32) -> u32) {
    for pixel in image.pixels_mut() {
        let alpha = u32::from(pixel.0[3]);
        for channel in &mut pixel.0[..3] {
            *channel = apply(u32::from(*channel), alpha) as u8;
        }
    }
}

fn bleed(mut image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    let source = image.clone();
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        if pixel.0[3] != 0 {
            continue;
        }
        let (mut sum, mut weight) = ([0u32; 3], 0u32);
        for (nx, ny) in (y.saturating_sub(1)..=(y + 1).min(height - 1))
            .flat_map(|ny| (x.saturating_sub(1)..=(x + 1).min(width - 1)).map(move |nx| (nx, ny)))
        {
            let near = source.get_pixel(nx, ny).0;
            let alpha = u32::from(near[3]);
            for (total, channel) in sum.iter_mut().zip(near) {
                *total += u32::from(channel) * alpha;
            }
            weight += alpha;
        }
        for (channel, total) in pixel.0.iter_mut().zip(sum) {
            if let Some(value) = total.checked_div(weight) {
                *channel = value as u8;
            }
        }
    }
    image
}

pub fn decode(set: EmojiSet, codes: &[u32]) -> Vec<(u32, RgbaImage)> {
    let mut images: HashMap<u32, RgbaImage> = HashMap::new();
    let wanted =
        |code: u32| codes.contains(&code) || set.aliases.iter().any(|(_, frame)| *frame == code);
    for atlas in &set.atlases {
        let Ok(decoded) = image::load_from_memory(&atlas.png) else {
            continue;
        };
        let decoded = decoded.into_rgba8();
        let side = f64::from(atlas.count).sqrt() as u32;
        if side == 0 {
            continue;
        }
        let (cell_w, cell_h) = (decoded.width() / side, decoded.height() / side);
        if cell_w == 0 || cell_h == 0 {
            continue;
        }
        for index in 0..atlas.count {
            let code = atlas.start + index;
            if !wanted(code) {
                continue;
            }
            let cell = imageops::crop_imm(
                &decoded,
                index % side * cell_w,
                index / side * cell_h,
                cell_w,
                cell_h,
            )
            .to_image();
            if !blank(&cell) {
                images.insert(code, cell);
            }
        }
    }
    for (code, png) in &set.singles {
        let Ok(decoded) = image::load_from_memory(png) else {
            continue;
        };
        let decoded = decoded.into_rgba8();
        if blank(&decoded) {
            continue;
        }
        let longest = decoded.width().max(decoded.height());
        let single = if longest > SOURCE_MAX {
            sized(&decoded, SOURCE_MAX)
        } else {
            decoded
        };
        images.insert(*code, single);
    }
    for (code, frame) in &set.aliases {
        if let Some(image) = images.get(frame).cloned() {
            images.insert(*code, image);
        }
    }
    images
        .into_iter()
        .filter(|(code, _)| codes.contains(code))
        .collect()
}

pub fn visible(text: &str, cx: &App) -> String {
    let images = cx.try_global::<EmojiImages>();
    text.chars()
        .filter(|ch| {
            !is_emoji(*ch)
                || images.is_some_and(|images| images.sources.contains_key(&u32::from(*ch)))
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

pub fn label(rank: &Rank, cx: &App) -> SharedString {
    let kept = visible(&rank.label, cx);
    if kept.is_empty() {
        rank.name.clone().into()
    } else {
        kept.into()
    }
}

pub fn icon(ch: char, size: f32, window: &Window, cx: &mut App) -> Option<AnyElement> {
    let code = u32::from(ch);
    let side = (size * window.scale_factor()).round().max(1.0) as u32;
    let images = cx.default_global::<EmojiImages>();
    let image = match images.sized.get(&(code, side)) {
        Some(image) => image.clone(),
        None => {
            let image = to_render_image(bleed(sized(images.sources.get(&code)?, side)));
            images.sized.insert((code, side), image.clone());
            image
        }
    };
    Some(img(image).flex_none().size(px(size)).into_any_element())
}

pub fn rich(text: &str, size: f32, window: &Window, cx: &mut App) -> Div {
    let mut row = div().flex().flex_none().items_center().whitespace_nowrap();
    let mut run = String::new();
    for ch in text.chars() {
        if !is_emoji(ch) {
            run.push(ch);
            continue;
        }
        let Some(icon) = icon(ch, size, window, cx) else {
            continue;
        };
        if !run.is_empty() {
            row = row.child(std::mem::take(&mut run));
        }
        row = row.child(icon);
    }
    if !run.is_empty() {
        row = row.child(run);
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use ac_api::EmojiAtlas;

    fn atlas_png() -> Vec<u8> {
        let mut atlas = RgbaImage::new(4, 4);
        atlas.put_pixel(2, 0, image::Rgba([10, 20, 30, 255]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(atlas)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png");
        png
    }

    #[test]
    fn only_wanted_cells_are_cut() {
        let set = EmojiSet {
            atlases: vec![EmojiAtlas {
                start: 0xE100,
                count: 4,
                png: atlas_png(),
            }],
            ..EmojiSet::default()
        };
        let images = decode(set, &[0xE101, 0xE102]);
        assert_eq!(images.len(), 1);
        let (code, image) = &images[0];
        assert_eq!(*code, 0xE101);
        assert_eq!(image.dimensions(), (2, 2));
        assert_eq!(image.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn sizing_keeps_pixels_sharp_at_whole_multiples() {
        let mut source = RgbaImage::new(2, 2);
        source.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        let doubled = sized(&source, 4);
        assert_eq!(doubled.dimensions(), (4, 4));
        assert_eq!(doubled.get_pixel(1, 1).0, [255, 0, 0, 255]);
        assert_eq!(doubled.get_pixel(2, 2).0[3], 0);
        assert_eq!(sized(&source, 3).dimensions(), (3, 3));
    }

    #[test]
    fn transparent_pixels_take_neighbour_colour() {
        let mut source = RgbaImage::new(3, 1);
        source.put_pixel(0, 0, image::Rgba([200, 100, 50, 255]));
        let bled = bleed(source);
        assert_eq!(bled.get_pixel(1, 0).0, [200, 100, 50, 0]);
        assert_eq!(bled.get_pixel(2, 0).0, [0, 0, 0, 0]);
    }
}
