use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PunishmentKind {
    Ban,
    Mute,
}

impl PunishmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ban => "ban",
            Self::Mute => "mute",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub text: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    pub kind: PunishmentKind,
    pub reason: String,
    pub issued_by: String,
    pub issued_by_name: String,
    #[serde(default)]
    pub issued_by_display: Vec<Segment>,
    pub permanent: bool,
    pub expires_at: i64,
    pub price: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CabinetStatus {
    pub ban: Option<Violation>,
    pub mute: Option<Violation>,
}

impl CabinetStatus {
    pub fn clean(&self) -> bool {
        self.ban.is_none() && self.mute.is_none()
    }
}
