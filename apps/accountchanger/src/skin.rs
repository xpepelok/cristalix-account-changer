use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ac_skin::{
    Cape, OrbitCamera, PixelFormat, Pose, Scene, Skin, SkinRenderer, supersample_factor,
};
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use parking_lot::Mutex;
use smallvec::smallvec;

pub const STAGE_FRAME: Duration = Duration::from_millis(33);
pub const STAGE_IDLE: Duration = Duration::from_millis(250);

pub fn bgra_image(width: u32, height: u32, bgra: Vec<u8>) -> Option<Arc<RenderImage>> {
    let buffer = RgbaImage::from_raw(width, height, bgra)?;
    Some(Arc::new(RenderImage::new(smallvec![Frame::new(buffer)])))
}

pub fn to_render_image(mut image: RgbaImage) -> Arc<RenderImage> {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(smallvec![Frame::new(image)]))
}

pub fn stage_glow(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        let dx = (x as f32 + 0.5) / width as f32 - 0.5;
        let dy = (y as f32 + 0.5) / height as f32 - 0.08;
        let distance = ((dx / 1.2).powi(2) + (dy / 0.8).powi(2)).sqrt();
        let strength = (1.0 - distance / 0.62).clamp(0.0, 1.0);
        image::Rgba([59, 130, 232, (strength * 0.14 * 255.0).round() as u8])
    })
}

#[derive(Clone)]
pub enum Texture {
    Loading,
    Missing,
    Ready(Arc<Skin>),
}

#[derive(Default)]
pub struct Skins {
    textures: HashMap<String, Texture>,
    heads: HashMap<(String, u32), Arc<RenderImage>>,
}

impl Skins {
    pub fn texture(&self, uuid: &str) -> Option<&Texture> {
        self.textures.get(uuid)
    }

    pub fn begin(&mut self, uuid: &str) -> bool {
        if self.textures.contains_key(uuid) {
            return false;
        }
        self.textures.insert(uuid.to_owned(), Texture::Loading);
        true
    }

    pub fn finish(&mut self, uuid: String, bytes: Option<Vec<u8>>) {
        let texture = bytes
            .as_deref()
            .and_then(|bytes| Skin::from_png(bytes).ok())
            .map_or(Texture::Missing, |skin| Texture::Ready(Arc::new(skin)));
        self.heads.retain(|(owner, _), _| *owner != uuid);
        self.textures.insert(uuid, texture);
    }

    pub fn head(&mut self, uuid: &str, size: u32) -> Option<Arc<RenderImage>> {
        let Some(Texture::Ready(skin)) = self.textures.get(uuid) else {
            return None;
        };
        let key = (uuid.to_owned(), size);
        if let Some(image) = self.heads.get(&key) {
            return Some(image.clone());
        }
        let image = bgra_image(size, size, skin.head_avatar(size, PixelFormat::Bgra))?;
        self.heads.insert(key, image.clone());
        Some(image)
    }
}

pub struct FrameRequest {
    pub skin: Arc<Skin>,
    pub cape: Option<Arc<Cape>>,
    pub camera: OrbitCamera,
    pub seconds: f32,
    pub width: u32,
    pub height: u32,
    pub supersample: u32,
    pub renderer: Arc<Mutex<SkinRenderer>>,
}

impl FrameRequest {
    pub fn render(self) -> Option<Arc<RenderImage>> {
        let mut renderer = self.renderer.lock();
        let mut scene = Scene::new(&self.skin, self.cape.as_deref());
        scene.camera = self.camera;
        scene.pose = Pose::idle(self.seconds);
        let bytes = renderer
            .render(
                &scene,
                self.width,
                self.height,
                self.supersample,
                PixelFormat::Bgra,
            )
            .to_vec();
        bgra_image(self.width, self.height, bytes)
    }
}

pub struct Stage {
    pub camera: OrbitCamera,
    pub cape: Option<Arc<Cape>>,
    pub frame: Option<Arc<RenderImage>>,
    pub size: (f32, f32),
    pub grab: Option<(f32, f32)>,
    started: Instant,
    renderer: Arc<Mutex<SkinRenderer>>,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            camera: OrbitCamera::default(),
            cape: None,
            frame: None,
            size: (0.0, 0.0),
            grab: None,
            started: Instant::now(),
            renderer: Arc::new(Mutex::new(SkinRenderer::new())),
        }
    }
}

impl Stage {
    pub fn request(&self, skin: Arc<Skin>, scale: f32) -> Option<FrameRequest> {
        let (width, height) = self.size;
        let width = (width * scale).round() as u32;
        let height = (height * scale).round() as u32;
        (width > 0 && height > 0).then(|| FrameRequest {
            skin,
            cape: self.cape.clone(),
            camera: self.camera,
            seconds: self.started.elapsed().as_secs_f32(),
            width,
            height,
            supersample: supersample_factor(scale),
            renderer: self.renderer.clone(),
        })
    }

    pub fn drag_to(&mut self, x: f32, y: f32) {
        if let Some((last_x, last_y)) = self.grab {
            self.camera.drag(x - last_x, y - last_y, self.size.1);
        }
        self.grab = Some((x, y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    #[test]
    fn bgra_buffers_must_match_size() {
        assert!(bgra_image(2, 2, vec![0; 16]).is_some());
        assert!(bgra_image(2, 2, vec![0; 3]).is_none());
    }

    #[test]
    fn render_images_are_bgra() {
        let image = RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 4]));
        assert_eq!(to_render_image(image).as_bytes(0), Some(&[3, 2, 1, 4][..]));
    }

    #[test]
    fn stage_needs_size_before_rendering() {
        let mut stage = Stage::default();
        let mut png = Vec::new();
        {
            let encoder = png_encoder(&mut png);
            encoder
                .write_image(&[255; 64 * 64 * 4], 64, 64, image::ExtendedColorType::Rgba8)
                .unwrap();
        }
        let skin = Arc::new(Skin::from_png(&png).unwrap());
        assert!(stage.request(skin.clone(), 1.0).is_none());
        stage.size = (20.0, 30.0);
        let frame = stage.request(skin, 1.5).unwrap().render().unwrap();
        assert_eq!(frame.size(0).width.0, 30);
    }

    fn png_encoder(out: &mut Vec<u8>) -> image::codecs::png::PngEncoder<&mut Vec<u8>> {
        image::codecs::png::PngEncoder::new(out)
    }

    #[test]
    fn skins_cache_heads() {
        let mut skins = Skins::default();
        assert!(skins.begin("a"));
        assert!(!skins.begin("a"));
        let mut png = Vec::new();
        {
            png_encoder(&mut png)
                .write_image(&[200; 64 * 64 * 4], 64, 64, image::ExtendedColorType::Rgba8)
                .unwrap();
        }
        skins.finish("a".into(), Some(png));
        let first = skins.head("a", 32).unwrap();
        assert_eq!(first.id, skins.head("a", 32).unwrap().id);
    }
}
