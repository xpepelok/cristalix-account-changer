use std::borrow::Cow;

use gpui::{App, AssetSource, Hsla, Pixels, Result, SharedString, Svg, prelude::*, svg};

use crate::theme::FONT_FAMILY;

macro_rules! icons {
    ($($variant:ident => $file:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Icon {
            $($variant),+
        }

        impl Icon {
            pub const fn path(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!("icons/", $file, ".svg")),+
                }
            }
        }

        const ICON_FILES: &[(&str, &[u8])] = &[
            $((concat!("icons/", $file, ".svg"), include_bytes!(concat!("../assets/icons/", $file, ".svg")))),+
        ];
    };
}

icons! {
    ArrowUp => "arrow-up",
    Bell => "bell",
    Cape => "cape",
    Caret => "caret",
    CaretDownBold => "caret-down-bold",
    CaretUp => "caret-up",
    Check => "check",
    Close => "close",
    CloseCircle => "close-circle",
    Copy => "copy",
    Download => "download",
    Eye => "eye",
    EyeOff => "eye-off",
    Filter => "filter",
    Gear => "gear",
    Gavel => "gavel",
    Globe => "globe",
    Grip => "grip",
    GroupProfile => "group-profile",
    Heart => "heart",
    ImportUser => "import-user",
    Layers => "layers",
    Logo => "logo",
    LogsFrame => "logs-frame",
    Moon => "moon",
    Muted => "muted",
    Play => "play",
    PlaySolid => "play-solid",
    Plus => "plus",
    PlusBold => "plus-bold",
    Refresh => "refresh",
    Reset => "reset",
    Skin => "skin",
    SpinnerArc => "spinner-arc",
    SpinnerRing => "spinner-ring",
    Star => "star",
    StarSolid => "star-solid",
    Sun => "sun",
    Telegram => "telegram",
    Trash => "trash",
    Users => "users",
    Warn => "warn",
    WinClose => "win-close",
    WinMax => "win-max",
    WinMin => "win-min",
    WinRestore => "win-restore",
    XMark => "xmark",
}

const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Montserrat-Regular.ttf"),
    include_bytes!("../assets/fonts/Montserrat-Medium.ttf"),
    include_bytes!("../assets/fonts/Montserrat-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Montserrat-Bold.ttf"),
    include_bytes!("../assets/fonts/Montserrat-ExtraBold.ttf"),
];

pub const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICON_FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICON_FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::new_static(name))
            .collect())
    }
}

pub fn register_fonts(cx: &App) -> SharedString {
    let fonts = FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect();
    let registered = cx.text_system().add_fonts(fonts).is_ok()
        && cx
            .text_system()
            .all_font_names()
            .iter()
            .any(|name| name == FONT_FAMILY);
    if registered {
        SharedString::new_static(FONT_FAMILY)
    } else {
        SharedString::new_static("Segoe UI")
    }
}

pub fn icon(icon: Icon, size: impl Into<Pixels>, color: Hsla) -> Svg {
    let size = size.into();
    svg()
        .flex_none()
        .size(size)
        .path(icon.path())
        .text_color(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_embedded() {
        let assets = Assets;
        for (path, bytes) in ICON_FILES {
            assert!(!bytes.is_empty(), "{path} is empty");
            assert!(assets.load(path).unwrap().is_some());
        }
        assert!(assets.load(Icon::Logo.path()).unwrap().is_some());
    }
}
