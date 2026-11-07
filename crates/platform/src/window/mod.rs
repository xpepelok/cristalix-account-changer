#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod sys;
#[cfg(windows)]
#[path = "windows.rs"]
mod sys;

#[cfg(windows)]
pub(crate) use sys::for_each_window;
pub use sys::set_window_title_for_pid;
#[cfg(windows)]
pub use sys::{WindowHandle, focus_window, window_for_pid, window_titles_by_pid};

#[cfg(test)]
mod tests {
    use super::set_window_title_for_pid;

    #[test]
    fn invalid_title_requests_are_rejected() {
        assert!(!set_window_title_for_pid(0, "Cristalix"));
        assert!(!set_window_title_for_pid(std::process::id(), "   "));
    }

    #[cfg(windows)]
    #[test]
    fn enumeration_never_reports_empty_titles() {
        let titles = super::window_titles_by_pid();
        assert!(titles.values().flatten().all(|title| !title.is_empty()));
        assert!(super::window_for_pid(0).is_none());
    }
}
