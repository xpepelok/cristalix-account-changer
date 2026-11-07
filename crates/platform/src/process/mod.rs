#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod sys;
#[cfg(windows)]
#[path = "windows.rs"]
mod sys;

pub use sys::{close_game, command_line, processes};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::{command_line, processes};

    #[test]
    fn scan_sees_the_current_process() {
        let me = std::process::id();
        let found = processes().into_iter().find(|info| info.pid == me);
        assert!(found.is_some_and(|info| !info.name.is_empty()));
    }

    #[test]
    fn command_line_of_the_current_process_is_readable() {
        let line = command_line(std::process::id()).unwrap_or_default();
        let exe = std::env::current_exe().unwrap();
        let stem = exe.file_stem().unwrap().to_string_lossy().into_owned();
        assert!(line.contains(&stem), "{line:?} does not mention {stem:?}");
    }
}
