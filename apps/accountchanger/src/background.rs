use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ac_api::Background;
use gpui::RenderImage;
use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use image::imageops::FilterType;
use image::{AnimationDecoder, ImageFormat, RgbaImage};

use crate::skin::to_render_image;

const MAX_EDGE: u32 = 2560;
const ANIMATED_MAX_EDGE: u32 = 1920;
const ANIMATION_BUDGET: u64 = 160 * 1024 * 1024;
const MIN_DELAY: Duration = Duration::from_millis(20);
const FALLBACK_DELAY: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Load {
    pub file: String,
    pub blur: u8,
}

pub struct Prepared {
    frames: Vec<(Arc<RenderImage>, Duration)>,
    total: Duration,
}

impl Prepared {
    fn animated(&self) -> bool {
        self.frames.len() > 1
    }

    fn position(&self, elapsed: Duration) -> (usize, Duration) {
        let total = self.total.as_nanos().max(1);
        let mut offset = Duration::from_nanos((elapsed.as_nanos() % total) as u64);
        for (index, (_, delay)) in self.frames.iter().enumerate() {
            if offset < *delay {
                return (index, *delay - offset);
            }
            offset -= *delay;
        }
        (0, self.frames[0].1)
    }
}

#[derive(Default)]
pub struct Backdrop {
    loaded: Option<Load>,
    pending: Option<Load>,
    image: Option<Prepared>,
    started: Option<Instant>,
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

    pub fn finish(&mut self, load: Load, image: Option<Prepared>) {
        if self.pending.as_ref() != Some(&load) {
            return;
        }
        self.pending = None;
        self.loaded = Some(load);
        self.retire();
        self.started = Some(Instant::now());
        self.image = image;
    }

    fn retire(&mut self) {
        if let Some(prepared) = self.image.take() {
            self.retired
                .extend(prepared.frames.into_iter().map(|(frame, _)| frame));
        }
    }

    pub fn take_retired(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.retired)
    }

    pub fn animated(&self) -> bool {
        self.image.as_ref().is_some_and(Prepared::animated)
    }

    pub fn next_change(&self) -> Option<Duration> {
        let prepared = self.image.as_ref().filter(|prepared| prepared.animated())?;
        Some(prepared.position(self.started?.elapsed()).1)
    }

    pub fn image(&self, settings: &Background) -> Option<(Arc<RenderImage>, f32)> {
        settings.file.as_ref()?;
        let prepared = self.image.as_ref()?;
        let index = match self.started {
            Some(started) if prepared.animated() => prepared.position(started.elapsed()).0,
            _ => 0,
        };
        let frame = prepared.frames[index].0.clone();
        Some((frame, f32::from(settings.dim.min(100)) / 100.0))
    }
}

pub fn prepare(bytes: &[u8], blur: u8) -> Option<Prepared> {
    let frames = decode_animation(bytes).filter(|frames| frames.len() > 1);
    let frames = match frames {
        Some(frames) => fit_animation(frames),
        None => vec![(
            fit(image::load_from_memory(bytes).ok()?.into_rgba8(), MAX_EDGE),
            Duration::ZERO,
        )],
    };
    let frames: Vec<(Arc<RenderImage>, Duration)> = frames
        .into_iter()
        .map(|(mut rgba, delay)| {
            if blur > 0 {
                rgba = image::imageops::fast_blur(&rgba, f32::from(blur));
            }
            (to_render_image(rgba), delay)
        })
        .collect();
    let total = frames.iter().map(|(_, delay)| *delay).sum();
    Some(Prepared { frames, total })
}

fn decode_animation(bytes: &[u8]) -> Option<Vec<(RgbaImage, Duration)>> {
    let frames = match image::guess_format(bytes).ok()? {
        ImageFormat::Gif => GifDecoder::new(Cursor::new(bytes))
            .ok()?
            .into_frames()
            .collect_frames()
            .ok()?,
        ImageFormat::WebP => {
            let decoder = WebPDecoder::new(Cursor::new(bytes)).ok()?;
            if !decoder.has_animation() {
                return None;
            }
            decoder.into_frames().collect_frames().ok()?
        }
        _ => return None,
    };
    Some(
        frames
            .into_iter()
            .map(|frame| {
                let delay = Duration::from(frame.delay());
                let delay = if delay < MIN_DELAY {
                    FALLBACK_DELAY
                } else {
                    delay
                };
                (frame.into_buffer(), delay)
            })
            .collect(),
    )
}

fn fit_animation(frames: Vec<(RgbaImage, Duration)>) -> Vec<(RgbaImage, Duration)> {
    let (width, height) = frames[0].0.dimensions();
    let edge = animation_edge(width, height, frames.len());
    frames
        .into_iter()
        .map(|(frame, delay)| (fit(frame, edge), delay))
        .collect()
}

fn animation_edge(width: u32, height: u32, count: usize) -> u32 {
    let bytes = u64::from(width) * u64::from(height) * 4 * count as u64;
    let scale = (ANIMATION_BUDGET as f64 / bytes.max(1) as f64)
        .sqrt()
        .min(1.0);
    ((width.max(height) as f64 * scale) as u32).clamp(1, ANIMATED_MAX_EDGE)
}

fn fit(image: RgbaImage, max_edge: u32) -> RgbaImage {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    if longest <= max_edge {
        return image;
    }
    let ratio = max_edge as f32 / longest as f32;
    image::imageops::resize(
        &image,
        ((width as f32 * ratio) as u32).max(1),
        ((height as f32 * ratio) as u32).max(1),
        FilterType::Triangle,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame, Rgba};

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
        let mut bytes = Cursor::new(Vec::new());
        RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]))
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        let prepared = prepare(&bytes.into_inner(), 2).unwrap();
        assert!(!prepared.animated());
        assert_eq!(
            &prepared.frames[0].0.as_bytes(0).unwrap()[..4],
            &[30, 20, 10, 255]
        );
    }

    #[test]
    fn keeps_gif_frames_and_delays() {
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            for (shade, millis) in [(0, 40), (255, 120)] {
                let buffer = RgbaImage::from_pixel(4, 4, Rgba([shade, shade, shade, 255]));
                let delay = Delay::from_numer_denom_ms(millis, 1);
                encoder
                    .encode_frame(Frame::from_parts(buffer, 0, 0, delay))
                    .unwrap();
            }
        }
        let prepared = prepare(&bytes, 0).unwrap();
        assert!(prepared.animated());
        assert_eq!(prepared.total, Duration::from_millis(160));
        assert_eq!(prepared.position(Duration::from_millis(10)).0, 0);
        assert_eq!(prepared.position(Duration::from_millis(50)).0, 1);
        assert_eq!(prepared.position(Duration::from_millis(170)).0, 0);
    }

    #[test]
    fn large_animations_fit_the_budget() {
        let edge = animation_edge(1920, 1080, 120);
        let height = edge * 1080 / 1920;
        assert!(u64::from(edge) * u64::from(height) * 4 * 120 <= ANIMATION_BUDGET);
        assert_eq!(animation_edge(640, 360, 10), 640);
        assert_eq!(animation_edge(4000, 2000, 1), ANIMATED_MAX_EDGE);
    }
}
