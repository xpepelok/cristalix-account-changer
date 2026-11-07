use serde::{Deserialize, Serialize};

pub const GROUP_NAME_MAX_CHARS: usize = 24;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
    pub profile: String,
    pub pinned: bool,
    pub order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupSkip {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupLaunchProgress {
    pub active: bool,
    pub paused: bool,
    pub done: u32,
    pub total: u32,
    pub skipped: Vec<GroupSkip>,
}
