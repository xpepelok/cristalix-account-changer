use std::io::Cursor;

use png::{BitDepth, ColorType, Transformations};

use crate::error::SkinError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PixelFormat {
    #[default]
    Rgba,
    Bgra,
}

impl PixelFormat {
    pub(crate) fn write(self, out: &mut [u8; 4], rgba: [u8; 4]) {
        let [r, g, b, a] = rgba;
        *out = match self {
            Self::Rgba => rgba,
            Self::Bgra => [b, g, r, a],
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![[0; 4]; width as usize * height as usize],
        }
    }

    pub fn decode_png(bytes: &[u8]) -> Result<Self, SkinError> {
        let mut decoder = png::Decoder::new(Cursor::new(bytes));
        decoder
            .set_transformations(Transformations::normalize_to_color8() | Transformations::ALPHA);
        let mut reader = decoder.read_info()?;
        let mut buffer = vec![0; reader.output_buffer_size().unwrap_or_default()];
        let frame = reader.next_frame(&mut buffer)?;
        let data = &buffer[..frame.buffer_size()];
        let pixels = match (frame.color_type, frame.bit_depth) {
            (ColorType::Rgba, BitDepth::Eight) => data.as_chunks::<4>().0.to_vec(),
            (ColorType::Rgb, BitDepth::Eight) => data
                .as_chunks::<3>()
                .0
                .iter()
                .map(|&[r, g, b]| [r, g, b, 255])
                .collect(),
            (ColorType::GrayscaleAlpha, BitDepth::Eight) => data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[v, a]| [v, v, v, a])
                .collect(),
            (ColorType::Grayscale, BitDepth::Eight) => {
                data.iter().map(|&v| [v, v, v, 255]).collect()
            }
            _ => return Err(SkinError::UnsupportedFormat),
        };
        Ok(Self {
            width: frame.width,
            height: frame.height,
            pixels,
        })
    }

    pub(crate) fn get(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels[(y * self.width + x) as usize]
    }

    pub(crate) fn set(&mut self, x: u32, y: u32, pixel: [u8; 4]) {
        let width = self.width;
        self.pixels[(y * width + x) as usize] = pixel;
    }
}
