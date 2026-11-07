use std::collections::{BTreeSet, HashMap, HashSet};

use parking_lot::{Mutex, RwLock};

pub const DEFAULT_GAME_TITLE: &str = "Cristalix";

#[derive(Default)]
pub struct Games {
    titles: RwLock<BTreeSet<String>>,
    launchers: Mutex<HashSet<u32>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub game: Vec<u32>,
    pub live: HashSet<u32>,
    pub java: HashSet<u32>,
}

impl Games {
    pub fn allow_titles<'a>(&self, names: impl IntoIterator<Item = &'a str>) {
        let mut titles = self.titles.write();
        for name in names {
            let low = name.trim().to_lowercase();
            if !low.is_empty() {
                titles.insert(low);
            }
        }
    }

    pub fn allowed_titles(&self) -> Vec<String> {
        std::iter::once(DEFAULT_GAME_TITLE.to_owned())
            .chain(self.titles.read().iter().cloned())
            .collect()
    }

    pub fn is_game_title(&self, title: &str) -> bool {
        let low = title.trim().to_lowercase();
        low == "cristalix" || low.starts_with("cristalix ") || self.titles.read().contains(&low)
    }

    pub(crate) fn register_launcher(&self, pid: u32) {
        if pid != 0 {
            self.launchers.lock().insert(pid);
        }
    }

    pub(crate) fn unregister_launcher(&self, pid: u32) {
        self.launchers.lock().remove(&pid);
    }

    fn is_launcher(&self, pid: u32) -> bool {
        self.launchers.lock().contains(&pid)
    }

    pub fn game_pids(&self) -> Vec<u32> {
        self.scan().game
    }

    #[cfg(windows)]
    pub fn scan(&self) -> Scan {
        let titles = ac_platform::window_titles_by_pid();
        let java = java_pids();
        let mut scan = Scan::default();
        for (pid, window_titles) in &titles {
            if !java.contains(pid) || self.is_launcher(*pid) {
                continue;
            }
            scan.live.insert(*pid);
            if window_titles.iter().any(|title| self.is_game_title(title)) {
                scan.game.push(*pid);
            }
        }
        scan.game.sort_unstable();
        scan.java = java;
        scan
    }

    #[cfg(target_os = "linux")]
    pub fn scan(&self) -> Scan {
        let procs = crate::procfs::scan_procs(true);
        let mut scan = Scan::default();
        for proc in &procs {
            if !proc.is_java() {
                continue;
            }
            scan.java.insert(proc.pid);
            if proc.is_game() && !self.is_launcher(proc.pid) {
                scan.game.push(proc.pid);
                scan.live.insert(proc.pid);
            }
        }
        scan.game.sort_unstable();
        scan
    }

    pub fn java_descendants(&self, root: u32) -> HashSet<u32> {
        let java: HashSet<u32> = java_pids();
        descendants(root)
            .into_iter()
            .filter(|pid| java.contains(pid))
            .collect()
    }
}

#[cfg(windows)]
fn java_pids() -> HashSet<u32> {
    ac_platform::processes()
        .into_iter()
        .filter(|info| info.name.to_lowercase().contains("java"))
        .map(|info| info.pid)
        .collect()
}

#[cfg(target_os = "linux")]
fn java_pids() -> HashSet<u32> {
    crate::procfs::scan_procs(true)
        .into_iter()
        .filter(crate::procfs::ProcInfo::is_java)
        .map(|proc| proc.pid)
        .collect()
}

fn descendants(root: u32) -> HashSet<u32> {
    if root == 0 {
        return HashSet::new();
    }
    let parents: HashMap<u32, u32> = ac_platform::processes()
        .into_iter()
        .map(|info| (info.pid, info.ppid))
        .collect();
    parents
        .keys()
        .copied()
        .filter(|&pid| descends_from(&parents, pid, root))
        .collect()
}

fn descends_from(parents: &HashMap<u32, u32>, pid: u32, root: u32) -> bool {
    let mut seen = HashSet::new();
    let mut current = pid;
    while current != 0 && seen.insert(current) {
        if current == root {
            return true;
        }
        current = parents.get(&current).copied().unwrap_or(0);
    }
    false
}

pub fn process_tree(root: u32) -> Vec<u32> {
    let mut out = vec![root];
    out.extend(descendants(root).into_iter().filter(|&pid| pid != root));
    out
}

#[cfg(windows)]
pub async fn kill_tree(pid: u32) {
    if pid == 0 {
        return;
    }
    let mut command = crate::command::detached("taskkill");
    command.arg("/F");
    for target in process_tree(pid) {
        command.arg("/PID").arg(target.to_string());
    }
    let _ = command.status().await;
}

#[cfg(target_os = "linux")]
pub async fn kill_tree(pid: u32) {
    let Ok(target) = libc::pid_t::try_from(pid) else {
        return;
    };
    if target <= 0 {
        return;
    }
    if unsafe { libc::kill(-target, libc::SIGKILL) } != 0 {
        unsafe { libc::kill(target, libc::SIGKILL) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_match_default_prefix_and_allow_list() {
        let games = Games::default();
        assert!(games.is_game_title(" Cristalix "));
        assert!(games.is_game_title("cristalix 1.12.2"));
        assert!(!games.is_game_title("CristalixLauncher"));
        assert!(!games.is_game_title("Steve"));
        games.allow_titles(["  Steve ", "", "Alex"]);
        assert!(games.is_game_title("STEVE"));
        assert_eq!(games.allowed_titles(), ["Cristalix", "alex", "steve"]);
    }

    #[test]
    fn ancestry_walk_handles_cycles_and_orphans() {
        let parents = HashMap::from([(10, 1), (11, 10), (12, 11), (20, 21), (21, 20), (30, 99)]);
        assert!(descends_from(&parents, 12, 10));
        assert!(descends_from(&parents, 10, 10));
        assert!(!descends_from(&parents, 20, 10));
        assert!(!descends_from(&parents, 30, 10));
    }

    #[test]
    fn process_tree_starts_with_root() {
        let me = std::process::id();
        assert_eq!(process_tree(me)[0], me);
        assert!(descendants(0).is_empty());
    }
}
