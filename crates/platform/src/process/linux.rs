use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use super::ProcessInfo;

const CLOSE_GRACE: Duration = Duration::from_secs(10);
const CLOSE_POLL: Duration = Duration::from_millis(250);

pub fn processes() -> Vec<ProcessInfo> {
    let Ok(dir) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(read_stat)
        .collect()
}

fn read_stat(pid: u32) -> Option<ProcessInfo> {
    let raw = fs::read(format!("/proc/{pid}/stat")).ok()?;
    let open = raw.iter().position(|&byte| byte == b'(')?;
    let close = raw.iter().rposition(|&byte| byte == b')')?;
    if close < open {
        return None;
    }
    let rest = std::str::from_utf8(&raw[close + 1..]).ok()?;
    let ppid = rest.split_ascii_whitespace().nth(1)?.parse().ok()?;
    Some(ProcessInfo {
        pid,
        ppid,
        name: String::from_utf8_lossy(&raw[open + 1..close]).into_owned(),
    })
}

pub fn command_line(pid: u32) -> Option<String> {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let line = String::from_utf8_lossy(&raw).replace('\0', " ");
    Some(line.trim().to_owned())
}

pub fn close_game(pid: u32) -> bool {
    let Ok(target) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if target == 0 || !alive(target) || unsafe { libc::kill(target, libc::SIGTERM) } != 0 {
        return false;
    }
    let _ = thread::Builder::new()
        .name("close-game".into())
        .spawn(move || {
            let deadline = Instant::now() + CLOSE_GRACE;
            while Instant::now() < deadline {
                if !alive(target) {
                    return;
                }
                thread::sleep(CLOSE_POLL);
            }
            unsafe { libc::kill(target, libc::SIGKILL) };
        });
    true
}

fn alive(pid: libc::pid_t) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}
