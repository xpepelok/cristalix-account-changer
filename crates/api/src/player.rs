use serde::{Deserialize, Serialize};

pub const PLAYER_RANK_LABEL: &str = "PLAYER";
pub const PLAYER_RANK_COLOR: &str = "#8c8c8c";
const DONATE_RANK_COLOR: &str = "#e3a400";

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rank {
    pub label: String,
    pub color: String,
}

impl Rank {
    pub fn player() -> Self {
        Self {
            label: PLAYER_RANK_LABEL.to_owned(),
            color: PLAYER_RANK_COLOR.to_owned(),
        }
    }
}

fn first_non_empty<'a>(candidates: impl IntoIterator<Item = &'a str>, fallback: &str) -> String {
    candidates
        .into_iter()
        .find(|c| !c.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

impl PlayerInfo {
    fn staff_rank(&self) -> Option<Rank> {
        (!self.staff.is_empty() && self.staff != PLAYER_RANK_LABEL).then(|| Rank {
            label: first_non_empty([self.label.as_str()], &self.staff),
            color: first_non_empty([self.color.as_str()], PLAYER_RANK_COLOR),
        })
    }

    fn staffless(&self) -> bool {
        self.staff.is_empty() || self.staff == PLAYER_RANK_LABEL
    }

    fn donate_rank(&self) -> Option<Rank> {
        if self.donate.is_empty() || self.donate == "NO" {
            return None;
        }
        Some(if self.staffless() {
            Rank {
                label: first_non_empty([self.label.as_str()], &self.donate),
                color: first_non_empty(
                    [self.color.as_str(), self.donate_color.as_str()],
                    DONATE_RANK_COLOR,
                ),
            }
        } else {
            Rank {
                label: self.donate.replace("_PLUS", "+").replace('_', " "),
                color: first_non_empty([self.donate_color.as_str()], DONATE_RANK_COLOR),
            }
        })
    }

    pub fn primary_rank(&self) -> Option<Rank> {
        self.staff_rank().or_else(|| self.donate_rank())
    }

    pub fn ranks(&self) -> Vec<Rank> {
        self.staff_rank()
            .into_iter()
            .chain(self.donate_rank())
            .collect()
    }
}
