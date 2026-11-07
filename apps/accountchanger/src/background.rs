use std::sync::Arc;

use ac_api::Background;
use gpui::RenderImage;
use image::imageops::FilterType;

use crate::skin::to_render_image;

const MAX_EDGE: u32 = 2560;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Load {
    pub file: String,
    pub blur: u8,
}

#[derive(Default)]
pub struct Backdrop {
    loaded: Option<Load>,
    pending: Option<Load>,
    image: Option<Arc<RenderImage>>,
    retired: Vec<Arc<RenderImage>>,
}

impl Backdrop {
    pub fn plan(&mut self, settings: &Background) -> Option<Load> {
        let Some(file) = settings.file.clone() else {
            self.loaded = None;
            self.pending = None;
            self.retire();
            return None;
        };
        let wanted = Load {
            file,
            blur: settings.blur,
        };
        if self.loaded.as_ref() == Some(&wanted) || self.pending.as_ref() == Some(&wanted) {
            return None;
        }
        self.pending = Some(wanted.clone());
        Some(wanted)
    }

    pub fn finish(&mut self, load: Load, image: Option<Arc<RenderImage>>) {
        if self.pending.as_ref() != Some(&load) {
            return;
        }
        self.pending = None;
        self.loaded = Some(load);
        self.retire();
        self.image = image;
    }

    fn retire(&mut self) {
        if let Some(image) = self.image.take() {
            self.retired.push(image);
        }
    }

    pub fn take_retired(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.retired)
    }

    pub fn image(&self, settings: &Background) -> Option<(Arc<RenderImage>, f32)> {
        settings.file.as_ref()?;
        let image = self.image.clone()?;
        Some((image, f32::from(settings.dim.min(100)) / 100.0))
    }
}

pub fn prepare(bytes: &[u8], blur: u8) -> Option<Arc<RenderImage>> {
    let decoded = image::load_from_memory(bytes).ok()?;
    let longest = decoded.width().max(decoded.height());
    let scaled = if longest > MAX_EDGE {
        let ratio = MAX_EDGE as f32 / longest as f32;
        decoded.resize(
            (decoded.width() as f32 * ratio) as u32,
            (decoded.height() as f32 * ratio) as u32,
            FilterType::Triangle,
        )
    } else {
        decoded
    };
    let mut rgba = scaled.into_rgba8();
    if blur > 0 {
        rgba = image::imageops::fast_blur(&rgba, f32::from(blur));
    }
    Some(to_render_image(rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(file: Option<&str>, blur: u8) -> Background {
        Background {
            file: file.map(str::to_owned),
            dim: 55,
            blur,
        }
    }

    #[test]
    fn plans_only_changes() {
        let mut backdrop = Backdrop::default();
        assert!(backdrop.plan(&settings(None, 0)).is_none());
        let load = backdrop.plan(&settings(Some("a.png"), 4)).unwrap();
        assert!(backdrop.plan(&settings(Some("a.png"), 4)).is_none());
        backdrop.finish(load, None);
        assert!(backdrop.plan(&settings(Some("a.png"), 4)).is_none());
        assert!(backdrop.plan(&settings(Some("a.png"), 8)).is_some());
    }

    #[test]
    fn stale_results_are_dropped() {
        let mut backdrop = Backdrop::default();
        let first = backdrop.plan(&settings(Some("a.png"), 1)).unwrap();
        let second = backdrop.plan(&settings(Some("a.png"), 2)).unwrap();
        backdrop.finish(first, None);
        assert_eq!(backdrop.pending.as_ref(), Some(&second));
    }

    #[test]
    fn prepares_blurred_bgra() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 255]))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let image = prepare(&bytes.into_inner(), 2).unwrap();
        assert_eq!(&image.as_bytes(0).unwrap()[..4], &[30, 20, 10, 255]);
    }
}
