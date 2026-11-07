use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::{Notify, oneshot};
use tokio::time::Instant;

use crate::fsutil::read_tail;
use crate::host::{Host, short_uuid};

const DISCOVER_WINDOW: Duration = Duration::from_secs(3 * 60);
const CLAIMED_WINDOW: Duration = Duration::from_secs(12 * 60 * 60);
const DISCOVER_POLL: Duration = Duration::from_millis(10);
const FOLLOW_POLL: Duration = Duration::from_millis(120);
const MTIME_SLACK: Duration = Duration::from_secs(2);
const PENDING_LIMIT: usize = 300;
const RENAME_TIMEOUT: Duration = Duration::from_secs(5);
const RENAME_POLL: Duration = Duration::from_millis(15);
const USER_MARKER: &str = "setting user:";

#[derive(Default)]
struct Cursor {
    offset: u64,
    pending: VecDeque<String>,
}

impl Cursor {
    fn read_delta(&mut self, path: &Path) -> Vec<String> {
        let Ok((start, data)) = read_tail(path, self.offset) else {
            return Vec::new();
        };
        if start < self.offset {
            self.pending.clear();
        }
        let Some(end) = data.iter().rposition(|&byte| byte == b'\n') else {
            self.offset = start;
            return Vec::new();
        };
        self.offset = start + end as u64 + 1;
        String::from_utf8_lossy(&data[..end])
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
            .collect()
    }

    fn remember(&mut self, line: &str) {
        self.pending.push_back(line.to_owned());
        if self.pending.len() > PENDING_LIMIT {
            self.pending.pop_front();
        }
    }
}

pub(crate) fn launcher_log_user_matches(line: &str, expected: &str) -> bool {
    let Some(index) = line.to_ascii_lowercase().find(USER_MARKER) else {
        return false;
    };
    let user = line[index + USER_MARKER.len()..].trim();
    !user.is_empty() && user.to_lowercase() == expected.trim().to_lowercase()
}

pub(crate) fn unique_log_name(name: &str, uuid: &str) -> String {
    let safe: String = name
        .trim()
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            other => other,
        })
        .collect();
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    format!("{safe}-{stamp}-{}.log", short_uuid(uuid))
}

async fn rename_open_log(from: &Path, to: &Path) -> bool {
    let deadline = Instant::now() + RENAME_TIMEOUT;
    while Instant::now() < deadline {
        if std::fs::rename(from, to).is_ok() {
            return true;
        }
        tokio::time::sleep(RENAME_POLL).await;
    }
    false
}

fn candidate_logs(updates: &Path, since: SystemTime) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(updates) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("logs").join("latest.log"))
        .filter(|path| {
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .is_ok_and(|modified| modified >= since)
        })
        .collect()
}

pub(crate) struct LogTail<'a, H: Host> {
    pub host: &'a H,
    pub updates: &'a Path,
    pub client: &'a str,
    pub uuid: &'a str,
    pub name: &'a str,
    pub closed: Arc<Notify>,
}

impl<H: Host> LogTail<'_, H> {
    pub async fn run(self, ready: oneshot::Sender<bool>) {
        let session = self.host.log_begin(self.uuid, self.name).await;
        let mut ready = Some(ready);
        self.append(
            session,
            vec![format!(
                "[AccountChanger] Ожидание live-лога клиента: {}",
                self.client
            )],
        )
        .await;
        match self.discover(session).await {
            Ok((claimed, cursor)) => {
                if let Some(ready) = ready.take() {
                    let _ = ready.send(true);
                }
                self.follow(session, &claimed, cursor).await;
            }
            Err(()) => {
                if let Some(ready) = ready.take() {
                    let _ = ready.send(false);
                }
            }
        }
        if let Some(session) = session {
            self.host.log_finish(session).await;
        }
    }

    async fn append(&self, session: Option<i64>, lines: Vec<String>) {
        if let Some(session) = session
            && !lines.is_empty()
        {
            self.host.log_append(session, lines).await;
        }
    }

    async fn discover(&self, session: Option<i64>) -> Result<(PathBuf, Cursor), ()> {
        let started = SystemTime::now();
        let since = started.checked_sub(MTIME_SLACK).unwrap_or(started);
        let deadline = Instant::now() + DISCOVER_WINDOW;
        let mut cursors: HashMap<PathBuf, Cursor> = HashMap::new();
        while Instant::now() < deadline {
            for path in candidate_logs(self.updates, since) {
                let cursor = cursors.entry(path.clone()).or_default();
                let lines = cursor.read_delta(&path);
                let Some(hit) = lines
                    .iter()
                    .position(|line| launcher_log_user_matches(line, self.name))
                else {
                    for line in &lines {
                        cursor.remember(line);
                    }
                    continue;
                };
                for line in &lines[..=hit] {
                    cursor.remember(line);
                }
                let dir = path.parent().unwrap_or(self.updates);
                let target = dir.join(unique_log_name(self.name, self.uuid));
                if !rename_open_log(&path, &target).await {
                    self.append(
                        session,
                        vec![
                            "[AccountChanger] Не удалось закрепить latest.log: файл занят лаунчером"
                                .to_owned(),
                        ],
                    )
                    .await;
                    return Err(());
                }
                let mut backlog = vec![format!(
                    "[AccountChanger] Live-лог закреплён: {}",
                    target
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default()
                )];
                backlog.extend(cursor.pending.drain(..));
                backlog.extend(lines[hit + 1..].iter().cloned());
                self.append(session, backlog).await;
                let follow = Cursor {
                    offset: cursor.offset,
                    pending: VecDeque::new(),
                };
                return Ok((target, follow));
            }
            tokio::time::sleep(DISCOVER_POLL).await;
        }
        Err(())
    }

    async fn follow(&self, session: Option<i64>, path: &Path, mut cursor: Cursor) {
        let deadline = Instant::now() + CLAIMED_WINDOW;
        let closed = self.closed.notified();
        tokio::pin!(closed);
        loop {
            let lines = cursor.read_delta(path);
            self.append(session, lines).await;
            if Instant::now() >= deadline {
                return;
            }
            tokio::select! {
                () = tokio::time::sleep(FOLLOW_POLL) => {}
                () = &mut closed => {
                    let lines = cursor.read_delta(path);
                    self.append(session, lines).await;
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_marker_matches_case_insensitively() {
        assert!(launcher_log_user_matches(
            "[12:00:01] [main/INFO]: Setting user: Steve",
            " steve "
        ));
        assert!(!launcher_log_user_matches("Setting user: Alex", "Steve"));
        assert!(!launcher_log_user_matches("Setting user:   ", "Steve"));
        assert!(!launcher_log_user_matches("no marker", "Steve"));
    }

    #[test]
    fn log_names_are_filesystem_safe() {
        let name = unique_log_name(" a<b>:c ", "0123456789abcdef");
        assert!(name.starts_with("a_b__c-"));
        assert!(name.ends_with("-01234567.log"));
    }

    #[test]
    fn cursor_returns_complete_lines_and_resets_on_truncate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("latest.log");
        std::fs::write(&path, b"one\r\ntwo\npart").unwrap();
        let mut cursor = Cursor::default();
        assert_eq!(cursor.read_delta(&path), ["one", "two"]);
        assert!(cursor.read_delta(&path).is_empty());
        std::fs::write(&path, b"one\r\ntwo\npartial done\n").unwrap();
        assert_eq!(cursor.read_delta(&path), ["partial done"]);
        cursor.remember("kept");
        std::fs::write(&path, b"new\n").unwrap();
        assert_eq!(cursor.read_delta(&path), ["new"]);
        assert!(cursor.pending.is_empty());
    }
}
