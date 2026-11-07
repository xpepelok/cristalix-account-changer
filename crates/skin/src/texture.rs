use crate::error::SkinError;
use crate::image::RgbaImage;

const AMBIENT_LIGHT: f32 = 3.0 / std::f32::consts::PI;

const LEGACY_LIMB_COPIES: [[u32; 6]; 12] = [
    [4, 16, 4, 4, 20, 48],
    [8, 16, 4, 4, 24, 48],
    [0, 20, 4, 12, 24, 52],
    [4, 20, 4, 12, 20, 52],
    [8, 20, 4, 12, 16, 52],
    [12, 20, 4, 12, 28, 52],
    [44, 16, 4, 4, 36, 48],
    [48, 16, 4, 4, 40, 48],
    [40, 20, 4, 12, 40, 52],
    [44, 20, 4, 12, 36, 52],
    [48, 20, 4, 12, 32, 52],
    [52, 20, 4, 12, 44, 52],
];

const HAT_AREAS: [[u32; 4]; 6] = [
    [40, 0, 8, 8],
    [48, 0, 8, 8],
    [32, 8, 8, 8],
    [40, 8, 8, 8],
    [48, 8, 8, 8],
    [56, 8, 8, 8],
];

const OVERLAY_AREAS: [[u32; 4]; 30] = [
    [4, 32, 4, 4],
    [8, 32, 4, 4],
    [0, 36, 4, 12],
    [4, 36, 4, 12],
    [8, 36, 4, 12],
    [12, 36, 4, 12],
    [20, 32, 8, 4],
    [28, 32, 8, 4],
    [16, 36, 4, 12],
    [20, 36, 8, 12],
    [28, 36, 4, 12],
    [32, 36, 8, 12],
    [44, 32, 4, 4],
    [48, 32, 4, 4],
    [40, 36, 4, 12],
    [44, 36, 4, 12],
    [48, 36, 4, 12],
    [52, 36, 12, 12],
    [4, 48, 4, 4],
    [8, 48, 4, 4],
    [0, 52, 4, 12],
    [4, 52, 4, 12],
    [8, 52, 4, 12],
    [12, 52, 4, 12],
    [52, 48, 4, 4],
    [56, 48, 4, 4],
    [48, 52, 4, 12],
    [52, 52, 4, 12],
    [56, 52, 4, 12],
    [60, 52, 4, 12],
];

const SLIM_PROBES: [[u32; 4]; 4] = [
    [50, 16, 2, 4],
    [54, 20, 2, 12],
    [42, 48, 2, 4],
    [46, 52, 2, 12],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelType {
    Classic,
    Slim,
}

#[derive(Debug, Clone)]
pub(crate) struct Texels {
    pub width: u32,
    pub scale: u32,
    pub opaque: Vec<u32>,
    pub blended: Vec<u32>,
}

impl Texels {
    fn lit(image: &RgbaImage, scale: u32) -> Self {
        let light = |channel: u8| (f32::from(channel) * AMBIENT_LIGHT).round().min(255.0) as u32;
        let (opaque, blended) = image
            .pixels
            .iter()
            .map(|&[r, g, b, a]| {
                let rgb = if a == 0 {
                    [0, 0, 0]
                } else {
                    [light(r), light(g), light(b)]
                };
                let alpha = u32::from(a);
                let premultiply = |channel: u32| (channel * alpha + 127) / 255;
                (pack(rgb, 255), pack(rgb.map(premultiply), alpha))
            })
            .unzip();
        Self {
            width: image.width,
            scale,
            opaque,
            blended,
        }
    }
}

fn pack([r, g, b]: [u32; 3], a: u32) -> u32 {
    r | g << 8 | b << 16 | a << 24
}

#[derive(Debug, Clone)]
pub struct Skin {
    image: RgbaImage,
    texels: Texels,
    model: ModelType,
}

impl Skin {
    pub fn from_png(bytes: &[u8]) -> Result<Self, SkinError> {
        Self::from_image(RgbaImage::decode_png(bytes)?)
    }

    pub fn from_image(image: RgbaImage) -> Result<Self, SkinError> {
        let (width, height) = (image.width, image.height);
        let legacy = match (width, height) {
            (w, h) if w == h => false,
            (w, h) if w == 2 * h => true,
            _ => return Err(SkinError::SkinSize(width, height)),
        };
        if width == 0 || width % 64 != 0 {
            return Err(SkinError::SkinSize(width, height));
        }
        let scale = width / 64;
        let mut canvas = RgbaImage::new(width, width);
        canvas.pixels[..image.pixels.len()].copy_from_slice(&image.pixels);
        if legacy {
            convert_legacy(&mut canvas, scale);
        }
        let checked_height = if legacy { 32 } else { 64 };
        if !has_transparency(&canvas, scale, [0, 0, 64, checked_height]) {
            let overlays: &[[u32; 4]] = if legacy { &[] } else { &OVERLAY_AREAS };
            for &area in HAT_AREAS.iter().chain(overlays) {
                clear(&mut canvas, scale, area);
            }
        }
        let model = detect_model(&canvas, scale);
        Ok(Self {
            texels: Texels::lit(&canvas, scale),
            image,
            model,
        })
    }

    pub fn model(&self) -> ModelType {
        self.model
    }

    pub fn image(&self) -> &RgbaImage {
        &self.image
    }

    pub(crate) fn texels(&self) -> &Texels {
        &self.texels
    }
}

#[derive(Debug, Clone)]
pub struct Cape {
    texels: Texels,
}

impl Cape {
    pub fn from_png(bytes: &[u8]) -> Result<Self, SkinError> {
        Self::from_image(&RgbaImage::decode_png(bytes)?)
    }

    pub fn from_image(image: &RgbaImage) -> Result<Self, SkinError> {
        let (width, height) = (image.width, image.height);
        let scale = if width == 2 * height {
            width / 64
        } else if 17 * width == 22 * height {
            width / 22
        } else if 11 * width == 23 * height {
            width / 46
        } else {
            0
        };
        if scale == 0 {
            return Err(SkinError::CapeSize(width, height));
        }
        let mut canvas = RgbaImage::new(64 * scale, 32 * scale);
        for y in 0..height.min(canvas.height) {
            for x in 0..width.min(canvas.width) {
                canvas.set(x, y, image.get(x, y));
            }
        }
        Ok(Self {
            texels: Texels::lit(&canvas, scale),
        })
    }

    pub(crate) fn texels(&self) -> &Texels {
        &self.texels
    }
}

fn convert_legacy(canvas: &mut RgbaImage, scale: u32) {
    for [sx, sy, w, h, dx, dy] in LEGACY_LIMB_COPIES {
        for y in 0..h * scale {
            for x in 0..w * scale {
                let pixel = canvas.get(sx * scale + w * scale - 1 - x, sy * scale + y);
                canvas.set(dx * scale + x, dy * scale + y, pixel);
            }
        }
    }
}

fn area_pixels(
    canvas: &RgbaImage,
    scale: u32,
    [x, y, w, h]: [u32; 4],
) -> impl Iterator<Item = [u8; 4]> + '_ {
    (y * scale..(y + h) * scale)
        .flat_map(move |py| (x * scale..(x + w) * scale).map(move |px| canvas.get(px, py)))
}

fn has_transparency(canvas: &RgbaImage, scale: u32, area: [u32; 4]) -> bool {
    area_pixels(canvas, scale, area).any(|[.., a]| a != 255)
}

fn clear(canvas: &mut RgbaImage, scale: u32, [x, y, w, h]: [u32; 4]) {
    for py in y * scale..(y + h) * scale {
        for px in x * scale..(x + w) * scale {
            canvas.set(px, py, [0; 4]);
        }
    }
}

fn detect_model(canvas: &RgbaImage, scale: u32) -> ModelType {
    let all = |color: [u8; 4]| {
        SLIM_PROBES
            .iter()
            .all(|&area| area_pixels(canvas, scale, area).all(|pixel| pixel == color))
    };
    let slim = SLIM_PROBES
        .iter()
        .any(|&area| has_transparency(canvas, scale, area))
        || all([0, 0, 0, 255])
        || all([255, 255, 255, 255]);
    if slim {
        ModelType::Slim
    } else {
        ModelType::Classic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_skin(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage {
            width,
            height,
            pixels: vec![color; (width * height) as usize],
        }
    }

    #[test]
    fn rejects_bad_sizes() {
        assert!(matches!(
            Skin::from_image(opaque_skin(64, 48, [1, 2, 3, 255])),
            Err(SkinError::SkinSize(64, 48))
        ));
        assert!(matches!(
            Skin::from_image(opaque_skin(32, 32, [1, 2, 3, 255])),
            Err(SkinError::SkinSize(32, 32))
        ));
        assert!(matches!(
            Cape::from_image(&opaque_skin(10, 10, [0; 4])),
            Err(SkinError::CapeSize(10, 10))
        ));
    }

    #[test]
    fn fully_opaque_skin_loses_overlays_and_is_classic() {
        let skin = Skin::from_image(opaque_skin(64, 64, [10, 200, 30, 255])).unwrap();
        let texels = skin.texels();
        assert_eq!(skin.model(), ModelType::Classic);
        assert_eq!(texels.blended[(8 * 64 + 40) as usize], 0);
        assert_eq!(texels.blended[(40 * 64 + 20) as usize], 0);
        assert_eq!(
            texels.opaque[(8 * 64 + 8) as usize],
            pack([10, 191, 29], 255)
        );
    }

    #[test]
    fn solid_black_and_white_arms_count_as_slim() {
        assert_eq!(
            Skin::from_image(opaque_skin(64, 64, [0, 0, 0, 255]))
                .unwrap()
                .model(),
            ModelType::Slim
        );
        assert_eq!(
            Skin::from_image(opaque_skin(64, 64, [255; 4]))
                .unwrap()
                .model(),
            ModelType::Slim
        );
    }

    #[test]
    fn legacy_skins_get_mirrored_left_limbs() {
        let mut image = opaque_skin(64, 32, [50, 50, 50, 255]);
        image.set(0, 20, [255, 0, 0, 255]);
        image.set(3, 20, [0, 0, 255, 255]);
        let skin = Skin::from_image(image).unwrap();
        let texels = skin.texels();
        assert_eq!(
            texels.opaque[(52 * 64 + 27) as usize],
            pack([244, 0, 0], 255)
        );
        assert_eq!(
            texels.opaque[(52 * 64 + 24) as usize],
            pack([0, 0, 244], 255)
        );
        assert_eq!(texels.blended[(48 * 64 + 4) as usize], 0);
        assert_eq!(skin.model(), ModelType::Classic);
        assert_eq!(skin.image().height, 32);
    }

    #[test]
    fn transparent_arm_slot_means_slim() {
        let mut image = opaque_skin(64, 64, [90, 90, 90, 255]);
        image.set(50, 16, [0; 4]);
        let skin = Skin::from_image(image).unwrap();
        assert_eq!(skin.model(), ModelType::Slim);
        assert_ne!(skin.texels().blended[(8 * 64 + 40) as usize], 0);
    }

    #[test]
    fn cape_formats_are_scaled_into_64x32() {
        let cape = Cape::from_image(&opaque_skin(22, 17, [255; 4])).unwrap();
        assert_eq!((cape.texels().width, cape.texels().scale), (64, 1));
        let hd = Cape::from_image(&opaque_skin(128, 64, [255; 4])).unwrap();
        assert_eq!((hd.texels().width, hd.texels().scale), (128, 2));
        let wide = Cape::from_image(&opaque_skin(46, 22, [255; 4])).unwrap();
        assert_eq!(wide.texels().scale, 1);
        assert_eq!(cape.texels().blended[63], 0);
    }

    #[test]
    fn premultiplies_partial_alpha() {
        let mut image = opaque_skin(64, 64, [200, 100, 0, 255]);
        image.set(40, 8, [200, 100, 0, 128]);
        let skin = Skin::from_image(image).unwrap();
        assert_eq!(
            skin.texels().blended[(8 * 64 + 40) as usize],
            pack([96, 48, 0], 128)
        );
    }
}
