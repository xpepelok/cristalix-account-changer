const EMOJI_FIRST: u32 = 0xE000;
const EMOJI_LAST: u32 = 0xF8FF;

pub fn is_emoji(ch: char) -> bool {
    (EMOJI_FIRST..=EMOJI_LAST).contains(&u32::from(ch))
}

pub fn emoji_codes(text: &str) -> impl Iterator<Item = u32> + '_ {
    text.chars().filter(|ch| is_emoji(*ch)).map(u32::from)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmojiAtlas {
    pub start: u32,
    pub count: u32,
    pub png: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmojiSet {
    pub atlases: Vec<EmojiAtlas>,
    pub singles: Vec<(u32, Vec<u8>)>,
    pub aliases: Vec<(u32, u32)>,
}
