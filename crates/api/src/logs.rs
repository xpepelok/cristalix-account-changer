use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSummary {
    pub uuid: String,
    pub name: String,
    pub lines: u64,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub at: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSession {
    pub id: String,
    pub started: i64,
    pub ended: i64,
    pub active: bool,
    pub lines: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogView {
    pub lines: Vec<LogLine>,
    pub sessions: Vec<LogSession>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStats {
    pub total_accounts: u64,
    pub total_sessions: u64,
    pub active_now: u64,
    pub total_duration: i64,
    pub avg_duration: i64,
    pub longest_duration: i64,
    pub longest_name: String,
    #[serde(rename = "longestUuid")]
    pub longest_uuid: String,
    pub most_launched_name: String,
    #[serde(rename = "mostLaunchedUuid")]
    pub most_launched_uuid: String,
    pub most_launched_count: u64,
    pub total_lines: u64,
}
