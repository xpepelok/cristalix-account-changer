use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportStatus {
    Pending,
    Working,
    Ok,
    Err,
    Skip,
    Canceled,
}

impl ImportStatus {
    pub const fn finished(self) -> bool {
        matches!(self, Self::Ok | Self::Err | Self::Skip | Self::Canceled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportItem {
    pub login: String,
    pub status: ImportStatus,
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportProgress {
    pub running: bool,
    pub total: u32,
    pub done: u32,
    pub ok: u32,
    pub fail: u32,
    pub skip: u32,
    pub items: Vec<ImportItem>,
}

impl ImportProgress {
    pub fn from_items(running: bool, items: Vec<ImportItem>) -> Self {
        let count = |pred: &dyn Fn(ImportStatus) -> bool| {
            items.iter().filter(|it| pred(it.status)).count() as u32
        };
        Self {
            running,
            total: items.len() as u32,
            done: count(&ImportStatus::finished),
            ok: count(&|s| s == ImportStatus::Ok),
            fail: count(&|s| s == ImportStatus::Err),
            skip: count(&|s| s == ImportStatus::Skip),
            items,
        }
    }

    pub fn has_results(&self) -> bool {
        !self.items.is_empty()
            && (self.running || self.done < self.total || self.ok > 0 || self.fail > 0)
    }

    pub fn fraction(&self) -> f32 {
        self.done as f32 / self.total.max(1) as f32
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub notes: String,
    pub name: String,
    pub published_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProgress {
    pub active: bool,
    pub done: bool,
    pub downloaded: u64,
    pub total: u64,
    pub error: String,
}

impl UpdateProgress {
    pub fn percent(&self) -> u8 {
        match self.total {
            0 => 0,
            total => (self.downloaded.min(total) * 100 / total) as u8,
        }
    }
}
