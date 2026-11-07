use crate::image::{PixelFormat, RgbaImage};
use crate::texture::Skin;

const BACKGROUND_SCALE: f32 = 8.0;
const FACE_POSITION: (f32, f32) = (0.1429, 0.1429);
const HAT_POSITION: (f32, f32) = (0.7143, 0.1429);

impl Skin {
    pub fn head_avatar(&self, size: u32, format: PixelFormat) -> Vec<u8> {
        let image = self.image();
        let mut out = vec![0; size as usize * size as usize * 4];
        if size == 0 {
            return out;
        }
        let face = Layer::new(image, size, FACE_POSITION);
        let hat = Layer::new(image, size, HAT_POSITION);
        for (index, pixel) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = (index % size as usize) as f32 + 0.5;
            let y = (index / size as usize) as f32 + 0.5;
            format.write(
                pixel,
                blend_over(hat.sample(image, x, y), face.sample(image, x, y)),
            );
        }
        out
    }
}

struct Layer {
    offset_x: f32,
    offset_y: f32,
    texels_per_pixel: f32,
}

impl Layer {
    fn new(image: &RgbaImage, size: u32, (position_x, position_y): (f32, f32)) -> Self {
        let size = size as f32;
        let drawn_width = size * BACKGROUND_SCALE;
        let drawn_height = drawn_width * image.height as f32 / image.width as f32;
        Self {
            offset_x: (size - drawn_width) * position_x,
            offset_y: (size - drawn_height) * position_y,
            texels_per_pixel: image.width as f32 / drawn_width,
        }
    }

    fn sample(&self, image: &RgbaImage, x: f32, y: f32) -> [u8; 4] {
        let u = ((x - self.offset_x) * self.texels_per_pixel).floor();
        let v = ((y - self.offset_y) * self.texels_per_pixel).floor();
        if u < 0.0 || v < 0.0 || u >= image.width as f32 || v >= image.height as f32 {
            return [0; 4];
        }
        image.get(u as u32, v as u32)
    }
}

fn blend_over(top: [u8; 4], bottom: [u8; 4]) -> [u8; 4] {
    let top_alpha = u32::from(top[3]);
    if top_alpha == 255 {
        return top;
    }
    if top_alpha == 0 {
        return bottom;
    }
    let bottom_weight = u32::from(bottom[3]) * (255 - top_alpha);
    let alpha = top_alpha * 255 + bottom_weight;
    if alpha == 0 {
        return [0; 4];
    }
    let mix = |channel: usize| {
        ((u32::from(top[channel]) * top_alpha * 255
            + u32::from(bottom[channel]) * bottom_weight
            + alpha / 2)
            / alpha) as u8
    };
    [mix(0), mix(1), mix(2), ((alpha + 127) / 255) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin_with_face(height: u32, hat: [u8; 4]) -> Skin {
        let mut image = RgbaImage::new(64, height);
        for y in 0..height {
            for x in 0..64 {
                image.set(x, y, [x as u8, y as u8, 0, 255]);
            }
        }
        for y in 8..16 {
            for x in 40..48 {
                image.set(x, y, hat);
            }
        }
        Skin::from_image(image).unwrap()
    }

    #[test]
    fn crops_face_texels_like_the_css() {
        let skin = skin_with_face(64, [0; 4]);
        let avatar = skin.head_avatar(8, PixelFormat::Rgba);
        for y in 0..8 {
            for x in 0..8 {
                let i = (y * 8 + x) * 4;
                assert_eq!(&avatar[i..i + 4], &[8 + x as u8, 8 + y as u8, 0, 255]);
            }
        }
        let big = skin.head_avatar(46, PixelFormat::Rgba);
        assert_eq!(&big[..4], &[8, 8, 0, 255]);
        let last = (46 * 46 - 1) * 4;
        assert_eq!(&big[last..last + 4], &[15, 15, 0, 255]);
    }

    #[test]
    fn hat_layer_is_composited_on_top() {
        let opaque = skin_with_face(64, [9, 9, 9, 255]).head_avatar(8, PixelFormat::Bgra);
        assert_eq!(&opaque[..4], &[9, 9, 9, 255]);
        let glass = skin_with_face(64, [255, 255, 255, 128]).head_avatar(8, PixelFormat::Rgba);
        assert_eq!(&glass[..4], &[132, 132, 128, 255]);
    }

    #[test]
    fn legacy_skins_follow_css_background_math() {
        let avatar = skin_with_face(32, [0; 4]).head_avatar(46, PixelFormat::Rgba);
        assert_eq!(&avatar[..4], &[40, 3, 0, 255]);
    }

    #[test]
    fn over_handles_transparent_layers() {
        assert_eq!(blend_over([0; 4], [0; 4]), [0; 4]);
        assert_eq!(blend_over([10, 20, 30, 128], [0; 4]), [10, 20, 30, 128]);
    }
}
