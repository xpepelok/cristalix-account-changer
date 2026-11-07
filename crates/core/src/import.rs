use ac_api::{ImportItem, ImportProgress, ImportStatus, msg};
use parking_lot::Mutex;

#[derive(Default)]
pub(crate) struct ImportJob {
    state: Mutex<JobState>,
}

#[derive(Default)]
struct JobState {
    running: bool,
    items: Vec<ImportItem>,
    cancelled: bool,
}

pub(crate) struct Credential {
    pub login: String,
    pub password: String,
}

pub(crate) enum StartResult {
    Started(Vec<Task>),
    Rejected(String),
}

#[cfg_attr(not(windows), expect(dead_code))]
pub(crate) struct Task {
    pub index: usize,
    pub login: String,
    pub secret: String,
    pub uuid: String,
}

impl ImportJob {
    pub fn running(&self) -> bool {
        self.state.lock().running
    }

    pub fn progress(&self) -> ImportProgress {
        let state = self.state.lock();
        ImportProgress::from_items(state.running, state.items.clone())
    }

    pub fn cancelled(&self) -> bool {
        self.state.lock().cancelled
    }

    pub fn request_cancel(&self) {
        let mut state = self.state.lock();
        if state.running {
            state.cancelled = true;
        }
    }

    pub fn finish(&self) {
        self.state.lock().running = false;
    }

    pub fn set_item(&self, index: usize, status: ImportStatus, text: &str, name: Option<&str>) {
        let mut state = self.state.lock();
        if let Some(item) = state.items.get_mut(index) {
            item.status = status;
            item.text = text.to_owned();
            if let Some(name) = name {
                item.name = name.to_owned();
            }
        }
    }

    pub fn mark_rest_cancelled(&self) {
        let mut state = self.state.lock();
        for item in &mut state.items {
            if matches!(item.status, ImportStatus::Pending | ImportStatus::Working) {
                item.status = ImportStatus::Canceled;
                item.text = msg::CANCELED.to_owned();
            }
        }
    }

    fn begin(&self, items: Vec<ImportItem>, tasks: Vec<Task>) -> StartResult {
        let mut state = self.state.lock();
        state.running = true;
        state.cancelled = false;
        state.items = items;
        StartResult::Started(tasks)
    }

    pub fn start_import(
        &self,
        credentials: Vec<Credential>,
        valid: &dyn Fn(&str) -> bool,
    ) -> StartResult {
        if self.running() {
            return StartResult::Rejected(msg::IMPORT_RUNNING.to_owned());
        }
        let mut items = Vec::new();
        let mut tasks = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = 0usize;
        let mut skipped = 0usize;
        for credential in credentials {
            let login = credential.login.trim().to_owned();
            if login.is_empty() || credential.password.is_empty() {
                continue;
            }
            let key = login.to_lowercase();
            if !seen.insert(key.clone()) {
                duplicates += 1;
                continue;
            }
            if valid(&key) {
                items.push(ImportItem {
                    login,
                    status: ImportStatus::Skip,
                    text: msg::IMPORT_SKIP_SAVED.to_owned(),
                    name: String::new(),
                });
                skipped += 1;
                continue;
            }
            tasks.push(Task {
                index: items.len(),
                login: login.clone(),
                secret: credential.password,
                uuid: String::new(),
            });
            items.push(ImportItem {
                login,
                status: ImportStatus::Pending,
                text: msg::IMPORT_QUEUED.to_owned(),
                name: String::new(),
            });
        }
        if tasks.is_empty() {
            return StartResult::Rejected(msg::import_nothing(skipped, duplicates));
        }
        self.begin(items, tasks)
    }

    pub fn start_refresh(&self, accounts: Vec<(String, String)>) -> StartResult {
        if self.running() {
            return StartResult::Rejected(msg::REFRESH_RUNNING.to_owned());
        }
        if accounts.is_empty() {
            return StartResult::Rejected(msg::REFRESH_NOTHING.to_owned());
        }
        let mut items = Vec::new();
        let mut tasks = Vec::new();
        for (uuid, name) in accounts {
            tasks.push(Task {
                index: items.len(),
                login: name.clone(),
                secret: String::new(),
                uuid,
            });
            items.push(ImportItem {
                login: name,
                status: ImportStatus::Pending,
                text: msg::IMPORT_QUEUED.to_owned(),
                name: String::new(),
            });
        }
        self.begin(items, tasks)
    }
}
