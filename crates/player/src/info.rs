use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::time::parse_rfc3339;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Presence {
    Online,
    Offline,
    Hidden,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerInfo {
    pub name: String,
    pub group: String,
    pub staff: String,
    pub donate: String,
    pub color: String,
    pub name_color: String,
    pub label: String,
    pub registered_at: String,
    pub online: Presence,
    pub realm: String,
    pub last_seen: String,
    pub subscription: String,
    pub sub_ending: String,
    pub likes: i64,
    pub views: i64,
    pub score: i64,
    pub donate_color: String,
}

impl PlayerInfo {
    pub fn last_seen_at(&self) -> Option<SystemTime> {
        parse_rfc3339(&self.last_seen)
    }

    pub fn registered_at_time(&self) -> Option<SystemTime> {
        parse_rfc3339(&self.registered_at)
    }

    pub fn subscription_label(&self) -> String {
        match self.subscription.strip_prefix("level_") {
            Some(level) if !level.is_empty() && level.bytes().all(|b| b.is_ascii_digit()) => {
                format!("Ур. {level}")
            }
            _ => self.subscription.clone(),
        }
    }

    pub(crate) fn forget_live_state(&mut self) {
        self.online = Presence::Unknown;
        self.realm.clear();
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SearchEntry {
    name: Option<String>,
    registered_at: Option<String>,
    groups: Option<Groups>,
    social: Option<Social>,
    status: Option<Status>,
    subscription: Option<Subscription>,
    stats: Option<Stats>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Groups {
    staff: Option<String>,
    donate: Option<String>,
    display: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Social {
    prefix: Option<String>,
    formatted_name: Option<String>,
    last_seen_online: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Status {
    online: Option<bool>,
    privacy_hidden: Option<bool>,
    realm: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Subscription {
    key: Option<String>,
    ending: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Stats {
    views: Option<i64>,
    likes: Option<i64>,
    score: Option<i64>,
}

impl SearchEntry {
    pub(crate) fn name(&self) -> &str {
        self.name.as_deref().unwrap_or_default()
    }

    pub(crate) fn into_info(self) -> PlayerInfo {
        let name = self.name.unwrap_or_default();
        let groups = self.groups.unwrap_or_default();
        let social = self.social.unwrap_or_default();
        let status = self.status.unwrap_or_default();
        let subscription = self.subscription.unwrap_or_default();
        let stats = self.stats.unwrap_or_default();

        let (color, label) = parse_formatted(social.prefix.as_deref().unwrap_or_default());
        let online = if status.privacy_hidden.unwrap_or_default() {
            Presence::Hidden
        } else {
            match status.online {
                Some(true) => Presence::Online,
                Some(false) => Presence::Offline,
                None => Presence::Unknown,
            }
        };
        let donate = groups.donate.unwrap_or_default();

        PlayerInfo {
            name_color: color_of_name(social.formatted_name.as_deref().unwrap_or_default(), &name),
            donate_color: donate_color(&donate).unwrap_or_default().to_owned(),
            name,
            group: groups.display.unwrap_or_default(),
            staff: groups.staff.unwrap_or_default(),
            donate,
            color,
            label,
            registered_at: self.registered_at.unwrap_or_default(),
            online,
            realm: status.realm.unwrap_or_default(),
            last_seen: social.last_seen_online.unwrap_or_default(),
            subscription: subscription.key.unwrap_or_default(),
            sub_ending: subscription.ending.unwrap_or_default(),
            likes: stats.likes.unwrap_or_default(),
            views: stats.views.unwrap_or_default(),
            score: stats.score.unwrap_or_default(),
        }
    }
}

const HEX_MARK: char = '\u{a8}';
const LEGACY_MARK: char = '§';

fn legacy_color(code: char) -> Option<&'static str> {
    Some(match code.to_ascii_lowercase() {
        '0' => "#4a4a4a",
        '1' => "#3b5bdb",
        '2' => "#2fb344",
        '3' => "#22b8cf",
        '4' => "#e03131",
        '5' => "#ae3ec9",
        '6' => "#f59f00",
        '7' => "#adb5bd",
        '8' => "#868e96",
        '9' => "#4d7cff",
        'a' => "#51cf66",
        'b' => "#3bc9db",
        'c' => "#ff6b6b",
        'd' => "#f06595",
        'e' => "#ffd43b",
        'f' => "#ffffff",
        _ => return None,
    })
}

pub(crate) fn donate_color(group: &str) -> Option<&'static str> {
    let normalized: String = group
        .chars()
        .flat_map(char::to_uppercase)
        .filter(char::is_ascii_uppercase)
        .collect();
    Some(match normalized.as_str() {
        "IRON" => "#aaaaaa",
        "VIP" | "VIPPLUS" | "GOLD" => "#ffff55",
        "PREMIUM" | "PREMIUMPLUS" => "#55ff55",
        "DIAMOND" | "MVP" | "MVPPLUS" => "#55ffff",
        "EMERALD" => "#00aa00",
        "SPONSOR" => "#ffaa00",
        _ => return None,
    })
}

fn hex_code(chars: &[char], at: usize) -> Option<String> {
    let code = chars.get(at + 1..at + 7)?;
    code.iter().all(char::is_ascii_hexdigit).then(|| {
        std::iter::once('#')
            .chain(code.iter().map(char::to_ascii_lowercase))
            .collect()
    })
}

pub(crate) fn parse_formatted(text: &str) -> (String, String) {
    let chars: Vec<char> = text.chars().collect();
    let mut color = None;
    let mut plain = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if current == HEX_MARK
            && let Some(hex) = hex_code(&chars, index)
        {
            color.get_or_insert(hex);
            index += 7;
            continue;
        }
        if current == LEGACY_MARK && index + 1 < chars.len() {
            if color.is_none() {
                color = legacy_color(chars[index + 1]).map(str::to_owned);
            }
            index += 2;
            continue;
        }
        plain.push(current);
        index += 1;
    }
    (color.unwrap_or_default(), plain.trim().to_owned())
}

pub fn formatted_segments(text: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut segments: Vec<(String, String)> = Vec::new();
    let mut color = String::new();
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if current == HEX_MARK
            && let Some(hex) = hex_code(&chars, index)
        {
            color = hex;
            index += 7;
            continue;
        }
        if current == LEGACY_MARK && index + 1 < chars.len() {
            let code = chars[index + 1];
            if let Some(legacy) = legacy_color(code) {
                color = legacy.to_owned();
            } else if code.eq_ignore_ascii_case(&'r') {
                color.clear();
            }
            index += 2;
            continue;
        }
        match segments.last_mut() {
            Some((part, last)) if *last == color => part.push(current),
            _ => segments.push((current.to_string(), color.clone())),
        }
        index += 1;
    }
    if let Some((part, _)) = segments.first_mut() {
        *part = part.trim_start().to_owned();
    }
    if let Some((part, _)) = segments.last_mut() {
        *part = part.trim_end().to_owned();
    }
    segments.retain(|(part, _)| !part.is_empty());
    segments
}

pub(crate) fn color_of_name(formatted: &str, name: &str) -> String {
    let chars: Vec<char> = formatted.chars().collect();
    let target: Vec<char> = name.chars().collect();
    let mut color = String::new();
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if current == HEX_MARK
            && let Some(hex) = hex_code(&chars, index)
        {
            color = hex;
            index += 7;
            continue;
        }
        if current == LEGACY_MARK && index + 1 < chars.len() {
            index += 2;
            continue;
        }
        if chars[index..].starts_with(&target) {
            break;
        }
        index += 1;
    }
    color
}

pub(crate) fn names_match(left: &str, right: &str) -> bool {
    left.chars()
        .flat_map(char::to_lowercase)
        .eq(right.chars().flat_map(char::to_lowercase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_hex_color_wins_and_codes_are_stripped() {
        assert_eq!(
            parse_formatted("\u{a8}4D7CFF§l Sr.Dev \u{a8}ffffff"),
            ("#4d7cff".to_owned(), "Sr.Dev".to_owned())
        );
    }

    #[test]
    fn prefix_legacy_color() {
        assert_eq!(
            parse_formatted("§9Sr.Dev"),
            ("#4d7cff".to_owned(), "Sr.Dev".to_owned())
        );
        assert_eq!(
            parse_formatted("§LBold§cRed"),
            ("#ff6b6b".to_owned(), "BoldRed".to_owned())
        );
        assert_eq!(parse_formatted("§"), (String::new(), "§".to_owned()));
    }

    #[test]
    fn formatted_name_keeps_colors_per_segment() {
        assert_eq!(
            formatted_segments("§eV §8┃ §eBelieveInDream"),
            vec![
                ("V ".to_owned(), "#ffd43b".to_owned()),
                ("┃ ".to_owned(), "#868e96".to_owned()),
                ("BelieveInDream".to_owned(), "#ffd43b".to_owned()),
            ]
        );
        assert_eq!(
            formatted_segments("\u{a8}60C3A2§lxpepelok§r!"),
            vec![
                ("xpepelok".to_owned(), "#60c3a2".to_owned()),
                ("!".to_owned(), String::new()),
            ]
        );
    }

    #[test]
    fn broken_hex_marker_is_kept_as_text() {
        assert_eq!(
            parse_formatted("\u{a8}12zz56ab"),
            (String::new(), "\u{a8}12zz56ab".to_owned())
        );
        assert_eq!(
            parse_formatted("ab\u{a8}12345"),
            (String::new(), "ab\u{a8}12345".to_owned())
        );
    }

    #[test]
    fn name_color_is_last_hex_before_the_name() {
        assert_eq!(
            color_of_name(
                "\u{a8}111111[A] \u{a8}60C3A2§lxpepelok\u{a8}222222",
                "xpepelok"
            ),
            "#60c3a2"
        );
        assert_eq!(color_of_name("\u{a8}111111other", "xpepelok"), "#111111");
        assert_eq!(color_of_name("plain", "plain"), "");
    }

    #[test]
    fn donate_colors_ignore_case_and_separators() {
        assert_eq!(donate_color("vip_plus"), Some("#ffff55"));
        assert_eq!(donate_color("MVP+"), Some("#55ffff"));
        assert_eq!(donate_color("Emerald"), Some("#00aa00"));
        assert_eq!(donate_color("NO"), None);
    }

    #[test]
    fn subscription_level_label() {
        let mut info = PlayerInfo {
            subscription: "level_3".into(),
            ..PlayerInfo::default()
        };
        assert_eq!(info.subscription_label(), "Ур. 3");
        info.subscription = "level_".into();
        assert_eq!(info.subscription_label(), "level_");
        info.subscription = "premium".into();
        assert_eq!(info.subscription_label(), "premium");
    }

    #[test]
    fn search_entry_maps_like_go() {
        let raw = r#"{"playerId":"3e7839e2-a269-11ed-b93f-1cb72cb014ae","name":"xpepelok","registeredAt":"2023-02-01T19:47:22.168Z","groups":{"staff":"sr_dev","donate":"VIP_PLUS","display":"sr_dev"},"social":{"prefix":"§9Sr.Dev","formattedName":"§9[Sr.Dev] ¨60c3a2xpepelok","lastSeenOnline":"2026-11-05T00:09:14.659529231Z"},"status":{"online":false,"privacyHidden":true,"realm":null},"subscription":{"key":"level_2","ending":null},"stats":{"views":20,"likes":1,"score":1}}"#;
        let entry: SearchEntry = serde_json::from_str(raw).unwrap();
        let info = entry.into_info();
        assert_eq!(info.name, "xpepelok");
        assert_eq!(info.group, "sr_dev");
        assert_eq!(info.staff, "sr_dev");
        assert_eq!(info.color, "#4d7cff");
        assert_eq!(info.label, "Sr.Dev");
        assert_eq!(info.name_color, "#60c3a2");
        assert_eq!(info.donate_color, "#ffff55");
        assert_eq!(info.online, Presence::Hidden);
        assert_eq!(info.realm, "");
        assert_eq!(info.subscription, "level_2");
        assert_eq!((info.likes, info.views, info.score), (1, 20, 1));
        assert!(info.last_seen_at().is_some());
        assert!(info.registered_at_time().is_some());
    }

    #[test]
    fn online_flag_without_privacy() {
        let online: SearchEntry =
            serde_json::from_str(r#"{"name":"a","status":{"online":true,"realm":"PRS-1"}}"#)
                .unwrap();
        let info = online.into_info();
        assert_eq!(info.online, Presence::Online);
        assert_eq!(info.realm, "PRS-1");
        let unknown: SearchEntry = serde_json::from_str(r#"{"name":"a"}"#).unwrap();
        assert_eq!(unknown.into_info().online, Presence::Unknown);
    }

    #[test]
    fn info_json_matches_go_shape() {
        let info = PlayerInfo {
            name: "a".into(),
            online: Presence::Offline,
            ..PlayerInfo::default()
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["online"], "offline");
        assert_eq!(json["nameColor"], "");
        let legacy: PlayerInfo = serde_json::from_str(r#"{"name":"a","online":""}"#).unwrap();
        assert_eq!(legacy.online, Presence::Unknown);
    }

    #[test]
    fn unicode_case_folding_for_names() {
        assert!(names_match("XPEPELOK", "xpepelok"));
        assert!(!names_match("xpepelok1", "xpepelok"));
    }
}
