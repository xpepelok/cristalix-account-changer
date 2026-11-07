use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const MINIMAL_PROFILE: &str = "Минимальные настройки";
pub const PROFILE_FILES: [&str; 4] = [
    "binds.json",
    "options.txt",
    "optionsof.txt",
    "voicechat.json",
];
pub const PROFILE_NAME_MAX_CHARS: usize = 40;

pub type ProfileFiles = BTreeMap<String, String>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profiles {
    pub profiles: Vec<String>,
    pub files: Vec<String>,
    pub builtin: Vec<String>,
}

impl Profiles {
    pub fn is_builtin(&self, name: &str) -> bool {
        self.builtin.iter().any(|b| b == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedProfile {
    pub name: String,
    pub copied: u32,
}
