use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::games::{Games, Scan};
use crate::host::{Host, LaunchRecord, now};

const WINDOW_GRACE: i64 = 20;
const SEEN_REFRESH: i64 = 30;
pub(crate) const LAUNCHING_WINDOW: i64 = 2 * 60;
const BIND_TIMEOUT: Duration = Duration::from_secs(90);
const BIND_POLL: Duration = Duration::from_millis(600);
const VERIFIED_POLL: Duration = Duration::from_millis(300);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presence {
    pub running: HashMap<String, u32>,
    pub launching: HashSet<String>,
}

impl Presence {
    pub fn pid(&self, uuid: &str) -> Option<u32> {
        self.running.get(uuid).copied().filter(|pid| *pid != 0)
    }

    pub fn is_running(&self, uuid: &str) -> bool {
        self.pid(uuid).is_some()
    }
}

pub struct GameTracker {
    games: Arc<Games>,
    launched: Mutex<Vec<LaunchRecord>>,
    saves: watch::Sender<Vec<LaunchRecord>>,
    presence: watch::Sender<Presence>,
}

impl GameTracker {
    pub fn new(games: Arc<Games>, records: Vec<LaunchRecord>) -> Self {
        Self {
            games,
            saves: watch::Sender::new(records.clone()),
            launched: Mutex::new(records),
            presence: watch::Sender::new(Presence::default()),
        }
    }

    pub fn games(&self) -> &Arc<Games> {
        &self.games
    }

    pub fn subscribe(&self) -> watch::Receiver<Presence> {
        self.presence.subscribe()
    }

    pub fn presence(&self) -> Presence {
        self.presence.borrow().clone()
    }

    pub async fn persist_loop<H: Host>(&self, host: &H) {
        let mut saves = self.saves.subscribe();
        while saves.changed().await.is_ok() {
            let records = saves.borrow_and_update().clone();
            host.save_launches(records).await;
        }
    }

    fn persist(&self, records: &[LaunchRecord]) {
        self.saves.send_replace(records.to_vec());
    }

    pub fn seed(&self, accounts: impl IntoIterator<Item = (String, i64)>) {
        let pids = self.games.game_pids();
        let mut launched = self.launched.lock();
        if seed_records(&mut launched, &pids, accounts.into_iter().collect()) {
            self.persist(&launched);
        }
    }

    pub fn note_launch(&self, uuid: &str) {
        let before = self.games.game_pids();
        let mut launched = self.launched.lock();
        launched.retain(|record| record.uuid != uuid);
        launched.push(LaunchRecord {
            uuid: uuid.to_owned(),
            launched_at: now(),
            pid: 0,
            seen_at: None,
            before_pids: before,
        });
        self.persist(&launched);
    }

    pub fn forget(&self, uuid: &str) {
        let mut launched = self.launched.lock();
        launched.retain(|record| record.uuid != uuid);
        self.persist(&launched);
    }

    pub fn resolve(&self) -> Presence {
        let scan = self.games.scan();
        let presence = {
            let mut launched = self.launched.lock();
            let (presence, changed) = resolve_records(&mut launched, &scan, now());
            if changed {
                self.persist(&launched);
            }
            presence
        };
        self.presence.send_if_modified(|current| {
            if *current == presence {
                return false;
            }
            *current = presence.clone();
            true
        });
        presence
    }

    pub async fn bind_game(&self, uuid: &str, before: &[u32]) -> u32 {
        let deadline = Instant::now() + BIND_TIMEOUT;
        let before: HashSet<u32> = before.iter().copied().collect();
        loop {
            let pids = self.games.game_pids();
            {
                let mut launched = self.launched.lock();
                if let Some((pid, claimed)) =
                    claim_new_game(&mut launched, uuid, &pids, &before, now())
                {
                    if claimed {
                        self.persist(&launched);
                    }
                    return pid;
                }
            }
            if Instant::now() > deadline {
                return 0;
            }
            tokio::time::sleep(BIND_POLL).await;
        }
    }

    pub async fn bind_verified_launcher(&self, uuid: &str, launcher_pid: u32) -> u32 {
        let deadline = Instant::now() + BIND_TIMEOUT;
        loop {
            let children = self.games.java_descendants(launcher_pid);
            for pid in self.games.game_pids() {
                if !children.contains(&pid) {
                    continue;
                }
                let mut launched = self.launched.lock();
                if let Some(record) = launched.iter_mut().find(|record| record.uuid == uuid) {
                    let stamp = now();
                    record.pid = pid;
                    record.launched_at = stamp;
                    record.seen_at = Some(stamp);
                    self.persist(&launched);
                    return pid;
                }
            }
            if Instant::now() > deadline {
                return 0;
            }
            tokio::time::sleep(VERIFIED_POLL).await;
        }
    }
}

fn seed_records(
    launched: &mut Vec<LaunchRecord>,
    game_pids: &[u32],
    mut accounts: Vec<(String, i64)>,
) -> bool {
    let claimed: HashSet<u32> = launched
        .iter()
        .map(|record| record.pid)
        .filter(|pid| *pid != 0)
        .collect();
    let free: Vec<u32> = game_pids
        .iter()
        .copied()
        .filter(|pid| !claimed.contains(pid))
        .collect();
    if free.is_empty() {
        return false;
    }
    let tracked: HashSet<String> = launched.iter().map(|record| record.uuid.clone()).collect();
    accounts.sort_by_key(|account| std::cmp::Reverse(account.1));
    let mut free = free.into_iter();
    for (uuid, last_launched) in accounts {
        if last_launched == 0 || tracked.contains(&uuid) {
            continue;
        }
        let Some(pid) = free.next() else {
            break;
        };
        launched.push(LaunchRecord {
            uuid,
            launched_at: last_launched,
            pid,
            seen_at: None,
            before_pids: Vec::new(),
        });
    }
    true
}

fn claim_new_game(
    launched: &mut Vec<LaunchRecord>,
    uuid: &str,
    pids: &[u32],
    before: &HashSet<u32>,
    now: i64,
) -> Option<(u32, bool)> {
    let live: HashSet<u32> = pids.iter().copied().collect();
    if let Some(record) = launched
        .iter()
        .find(|record| record.uuid == uuid && record.pid != 0 && live.contains(&record.pid))
    {
        return Some((record.pid, false));
    }
    let claimed: HashSet<u32> = launched
        .iter()
        .map(|record| record.pid)
        .filter(|pid| *pid != 0)
        .collect();
    let found = pids
        .iter()
        .copied()
        .find(|pid| !before.contains(pid) && !claimed.contains(pid))?;
    match launched.iter_mut().find(|record| record.uuid == uuid) {
        Some(record) => {
            record.pid = found;
            record.launched_at = now;
            record.seen_at = Some(now);
        }
        None => launched.push(LaunchRecord {
            uuid: uuid.to_owned(),
            launched_at: now,
            pid: found,
            seen_at: Some(now),
            before_pids: Vec::new(),
        }),
    }
    Some((found, true))
}

fn resolve_records(launched: &mut Vec<LaunchRecord>, scan: &Scan, now: i64) -> (Presence, bool) {
    launched.sort_by_key(|record| record.launched_at);
    let mut presence = Presence::default();
    let mut claimed = HashSet::new();
    let mut changed = false;

    for record in launched.iter_mut() {
        if record.pid == 0 || claimed.contains(&record.pid) {
            if record.pid != 0 {
                record.pid = 0;
                changed = true;
            }
            continue;
        }
        if scan.live.contains(&record.pid) {
            if record.seen_at.is_none_or(|seen| now - seen > SEEN_REFRESH) {
                changed = true;
            }
            record.seen_at = Some(now);
            claimed.insert(record.pid);
            presence.running.insert(record.uuid.clone(), record.pid);
            continue;
        }
        if scan.java.contains(&record.pid)
            && record.seen_at.is_some_and(|seen| now - seen < WINDOW_GRACE)
        {
            claimed.insert(record.pid);
            presence.running.insert(record.uuid.clone(), record.pid);
            continue;
        }
        record.pid = 0;
        changed = true;
    }

    let free: Vec<u32> = scan
        .game
        .iter()
        .copied()
        .filter(|pid| !claimed.contains(pid))
        .collect();
    for record in launched.iter_mut().filter(|record| record.pid == 0) {
        let Some(pid) = free
            .iter()
            .copied()
            .find(|pid| *pid != 0 && !claimed.contains(pid) && !record.before_pids.contains(pid))
        else {
            continue;
        };
        record.pid = pid;
        record.seen_at = Some(now);
        claimed.insert(pid);
        presence.running.insert(record.uuid.clone(), pid);
        changed = true;
    }

    launched.retain(|record| {
        if record.pid != 0 {
            return true;
        }
        if now - record.launched_at < LAUNCHING_WINDOW {
            presence.launching.insert(record.uuid.clone());
            return true;
        }
        changed = true;
        false
    });
    (presence, changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(uuid: &str, at: i64, pid: u32, seen: Option<i64>, before: &[u32]) -> LaunchRecord {
        LaunchRecord {
            uuid: uuid.to_owned(),
            launched_at: at,
            pid,
            seen_at: seen,
            before_pids: before.to_vec(),
        }
    }

    fn scan(game: &[u32], live: &[u32], java: &[u32]) -> Scan {
        Scan {
            game: game.to_vec(),
            live: live.iter().copied().collect(),
            java: java.iter().copied().collect(),
        }
    }

    #[test]
    fn new_windows_pair_with_pending_launches_in_launch_order() {
        let mut launched = vec![
            record("b", 200, 0, None, &[10]),
            record("a", 100, 0, None, &[10]),
        ];
        let window = scan(&[10, 20, 30], &[10, 20, 30], &[10, 20, 30]);
        let (presence, changed) = resolve_records(&mut launched, &window, 210);
        assert!(changed);
        assert_eq!(presence.pid("a"), Some(20));
        assert_eq!(presence.pid("b"), Some(30));
        assert!(presence.launching.is_empty());
    }

    #[test]
    fn windows_open_before_launch_are_never_claimed() {
        let mut launched = vec![record("a", 100, 0, None, &[10])];
        let (presence, _) = resolve_records(&mut launched, &scan(&[10], &[10], &[10]), 110);
        assert!(!presence.is_running("a"));
        assert!(presence.launching.contains("a"));
    }

    #[test]
    fn stale_launches_expire_and_duplicates_lose_their_pid() {
        let mut launched = vec![
            record("old", 0, 0, None, &[]),
            record("first", 1, 50, Some(1000), &[]),
            record("second", 2, 50, Some(1000), &[]),
        ];
        let (presence, changed) = resolve_records(&mut launched, &scan(&[50], &[50], &[50]), 1000);
        assert!(changed);
        assert_eq!(presence.pid("first"), Some(50));
        assert!(!presence.is_running("second"));
        assert!(!launched.iter().any(|r| r.uuid == "old"));
    }

    #[test]
    fn java_without_window_keeps_running_only_within_grace() {
        let mut launched = vec![record("a", 1, 7, Some(100), &[])];
        let hidden = scan(&[], &[], &[7]);
        let (presence, _) = resolve_records(&mut launched, &hidden, 110);
        assert_eq!(presence.pid("a"), Some(7));
        let (presence, _) = resolve_records(&mut launched, &hidden, 125);
        assert!(!presence.is_running("a"));
    }

    #[test]
    fn unchanged_live_games_do_not_persist_until_seen_goes_stale() {
        let mut launched = vec![record("a", 1, 7, Some(100), &[])];
        let live = scan(&[7], &[7], &[7]);
        assert!(!resolve_records(&mut launched, &live, 110).1);
        assert!(resolve_records(&mut launched, &live, 141).1);
    }

    #[test]
    fn bind_reuses_live_pid_or_claims_first_new_window() {
        let mut launched = vec![record("a", 1, 0, None, &[])];
        let before = HashSet::from([5]);
        assert_eq!(claim_new_game(&mut launched, "a", &[5], &before, 9), None);
        assert_eq!(
            claim_new_game(&mut launched, "a", &[5, 6], &before, 9),
            Some((6, true))
        );
        assert_eq!(
            claim_new_game(&mut launched, "a", &[5, 6], &before, 10),
            Some((6, false))
        );
        assert_eq!(
            claim_new_game(&mut launched, "b", &[5, 6, 8], &before, 11),
            Some((8, true))
        );
        assert_eq!(launched.len(), 2);
    }

    #[test]
    fn seed_assigns_free_windows_to_recent_untracked_accounts() {
        let mut launched = vec![record("tracked", 1, 3, Some(1), &[])];
        let accounts = vec![
            ("never".to_owned(), 0),
            ("older".to_owned(), 10),
            ("tracked".to_owned(), 50),
            ("newer".to_owned(), 20),
        ];
        assert!(seed_records(&mut launched, &[3, 4], accounts));
        assert_eq!(launched.len(), 2);
        assert_eq!(launched[1].uuid, "newer");
        assert_eq!(launched[1].pid, 4);
        assert!(!seed_records(&mut launched, &[3, 4], Vec::new()));
    }
}
