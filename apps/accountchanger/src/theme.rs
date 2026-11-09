use ac_api::Theme;
use gpui::{App, BoxShadow, Global, Hsla, Pixels, Rgba, point, px};

pub const FONT_FAMILY: &str = "Montserrat";
pub const LINE_HEIGHT: f32 = 1.55;
pub const RADIUS: f32 = 14.0;
pub const GUTTER: f32 = 24.0;
pub const GUTTER_COMPACT: f32 = 14.0;
pub const GUTTER_NARROW: f32 = 10.0;
pub const COMPACT_WIDTH: f32 = 620.0;
pub const NARROW_WIDTH: f32 = 520.0;

pub const BASE_TEXT: f32 = 15.0;

pub fn tabular(weight: gpui::FontWeight) -> gpui::Font {
    gpui::Font {
        features: gpui::FontFeatures(std::sync::Arc::new(vec![("tnum".to_owned(), 1)])),
        weight,
        ..gpui::font(FONT_FAMILY)
    }
}

pub fn rem(value: f32) -> Pixels {
    px(value * 16.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub theme: Theme,
    pub graphite: Hsla,
    pub surface: Hsla,
    pub surface_2: Hsla,
    pub line: Hsla,
    pub line_strong: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub blue: Hsla,
    pub blue_hover: Hsla,
    pub green: Hsla,
    pub online: Hsla,
    pub yellow: Hsla,
    pub red: Hsla,
    pub stop: Hsla,
    pub white: Hsla,
    pub wash: Hsla,
    pub active_text: Hsla,
    pub topbar: Hsla,
    pub scrim: Hsla,
    pub toast: Hsla,
    pub toast_error: Hsla,
    pub icon_hover_bg: Hsla,
    pub icon_hover_fg: Hsla,
    pub close_hover_bg: Hsla,
    pub close_hover_fg: Hsla,
    pub count_bg: Hsla,
    pub logs_bg: Hsla,
    pub logs_text: Hsla,
    pub popup_shadow: f32,
}

pub const fn hex(value: u32) -> Rgba {
    Rgba {
        r: ((value >> 16) & 0xff) as f32 / 255.0,
        g: ((value >> 8) & 0xff) as f32 / 255.0,
        b: (value & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

pub fn rgb(value: u32) -> Hsla {
    hex(value).into()
}

pub fn rgba(value: u32, alpha: f32) -> Hsla {
    Rgba {
        a: alpha,
        ..hex(value)
    }
    .into()
}

pub fn alpha(color: Hsla, value: f32) -> Hsla {
    Hsla {
        a: color.a * value,
        ..color
    }
}

pub fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return from;
    }
    if t >= 1.0 {
        return to;
    }
    let a: Rgba = from.into();
    let b: Rgba = to.into();
    let alpha = a.a + (b.a - a.a) * t;
    if alpha <= f32::EPSILON {
        return Hsla { a: 0.0, ..to };
    }
    let channel = |x: f32, y: f32| (x * a.a + (y * b.a - x * a.a) * t) / alpha;
    Rgba {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
        a: alpha,
    }
    .into()
}

pub fn parse_hex(raw: &str) -> Option<Hsla> {
    let digits = raw.trim().strip_prefix('#')?;
    let value = match digits.len() {
        6 => u32::from_str_radix(digits, 16).ok()?,
        3 => {
            let short = u32::from_str_radix(digits, 16).ok()?;
            let (r, g, b) = ((short >> 8) & 0xf, (short >> 4) & 0xf, short & 0xf);
            ((r * 17) << 16) | ((g * 17) << 8) | (b * 17)
        }
        _ => return None,
    };
    Some(rgb(value))
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            theme: Theme::Dark,
            graphite: rgb(0x161616),
            surface: rgb(0x1c1c1c),
            surface_2: rgb(0x222222),
            line: rgba(0xffffff, 0.07),
            line_strong: rgba(0xffffff, 0.12),
            text: rgb(0xf0f0f0),
            muted: rgb(0x8c8c8c),
            faint: rgb(0x5e5e5e),
            blue: rgb(0x3b82e8),
            blue_hover: rgb(0x2f74dc),
            green: rgb(0x1fa344),
            online: rgb(0x2ecc57),
            yellow: rgb(0xe3a400),
            red: rgb(0xe0564f),
            stop: rgb(0xd13b34),
            white: rgb(0xffffff),
            wash: rgba(0xffffff, 0.05),
            active_text: rgb(0xcfe0fb),
            topbar: rgba(0x161616, 0.72),
            scrim: rgba(0x000000, 0.66),
            toast: rgb(0x0e0e0e),
            toast_error: rgb(0xffb3ae),
            icon_hover_bg: rgb(0x2e2e2e),
            icon_hover_fg: rgb(0xf0f0f0),
            close_hover_bg: rgb(0x222222),
            close_hover_fg: rgb(0xf0f0f0),
            count_bg: rgba(0xffffff, 0.06),
            logs_bg: rgb(0x111111),
            logs_text: rgb(0xd6d6d6),
            popup_shadow: 0.45,
        }
    }

    pub fn light() -> Self {
        let blue = rgb(0x3b82e8);
        Self {
            theme: Theme::Light,
            graphite: rgb(0xeceef2),
            surface: rgb(0xffffff),
            surface_2: rgb(0xf4f5f8),
            line: rgba(0x000000, 0.09),
            line_strong: rgba(0x000000, 0.17),
            text: rgb(0x17181c),
            muted: rgb(0x5f636b),
            faint: rgb(0x969ba3),
            wash: rgba(0x000000, 0.05),
            active_text: rgb(0x1f5fc8),
            topbar: rgba(0xffffff, 0.72),
            toast: rgb(0xffffff),
            toast_error: rgb(0xb3241c),
            icon_hover_bg: blue,
            icon_hover_fg: rgb(0xffffff),
            close_hover_bg: blue,
            close_hover_fg: rgb(0xffffff),
            count_bg: rgba(0x000000, 0.06),
            logs_bg: rgb(0xf4f5f8),
            logs_text: rgb(0x2a2c31),
            popup_shadow: 0.18,
            ..Self::dark()
        }
    }

    pub fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }

    pub fn is_light(&self) -> bool {
        self.theme == Theme::Light
    }

    pub fn popup_shadow(&self) -> Vec<BoxShadow> {
        shadow(0.0, 14.0, 34.0, 0.0, rgba(0x000000, self.popup_shadow))
    }
}

pub fn shadow(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color,
        offset: point(px(x), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
    }]
}

pub struct ActivePalette(pub Palette);

impl Global for ActivePalette {}

pub trait ThemeExt {
    fn palette(&self) -> Palette;
}

impl ThemeExt for App {
    fn palette(&self) -> Palette {
        self.try_global::<ActivePalette>()
            .map(|active| active.0)
            .unwrap_or_else(Palette::dark)
    }
}

pub fn install(theme: Theme, cx: &mut App) {
    cx.set_global(ActivePalette(Palette::for_theme(theme)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_parse() {
        assert_eq!(parse_hex("#ffffff"), Some(rgb(0xffffff)));
        assert_eq!(parse_hex("#fff"), Some(rgb(0xffffff)));
        assert_eq!(parse_hex("e3a400"), None);
        assert_eq!(parse_hex("#zzzzzz"), None);
    }

    #[test]
    fn mix_fades_through_transparent_without_darkening() {
        let half: Rgba = mix(rgba(0x000000, 0.0), rgb(0xffffff), 0.5).into();
        assert!((half.a - 0.5).abs() < 1e-4);
        assert!(half.r > 0.999 && half.g > 0.999 && half.b > 0.999);
    }

    #[test]
    fn mix_hits_both_ends() {
        let a = rgb(0x000000);
        let b = rgb(0xffffff);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
    }

    #[test]
    fn light_theme_keeps_accents() {
        assert_eq!(Palette::light().blue, Palette::dark().blue);
        assert_ne!(Palette::light().surface, Palette::dark().surface);
    }
}
