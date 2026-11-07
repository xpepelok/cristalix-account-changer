use std::collections::HashMap;
use std::ffi::c_void;
use std::num::NonZeroIsize;
use std::ops::ControlFlow;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::AttachThreadInput;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, SMTO_ABORTIFHUNG, SW_RESTORE, SendMessageTimeoutW,
    SetForegroundWindow, ShowWindow, WM_SETTEXT,
};
use windows::core::BOOL;

use crate::win::to_wide;

const SET_TITLE_TIMEOUT_MS: u32 = 800;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowHandle(NonZeroIsize);

impl WindowHandle {
    fn from_hwnd(hwnd: HWND) -> Option<Self> {
        NonZeroIsize::new(hwnd.0 as isize).map(Self)
    }

    fn hwnd(self) -> HWND {
        HWND(self.0.get() as *mut c_void)
    }
}

pub(crate) struct Window {
    pub(crate) hwnd: HWND,
    pub(crate) pid: u32,
    title_len: usize,
}

impl Window {
    fn title(&self) -> String {
        let mut buffer = vec![0u16; self.title_len + 1];
        let copied = unsafe { GetWindowTextW(self.hwnd, &mut buffer) };
        let copied = usize::try_from(copied).unwrap_or(0).min(self.title_len);
        String::from_utf16_lossy(&buffer[..copied])
    }
}

type Visitor<'a> = &'a mut dyn FnMut(&Window) -> ControlFlow<()>;

pub(crate) fn for_each_window(mut visit: impl FnMut(&Window) -> ControlFlow<()>) {
    let mut visitor: Visitor<'_> = &mut visit;
    let context = LPARAM((&raw mut visitor) as isize);
    let _ = unsafe { EnumWindows(Some(enum_window), context) };
}

unsafe extern "system" fn enum_window(hwnd: HWND, context: LPARAM) -> BOOL {
    let visitor = unsafe { &mut *(context.0 as *mut Visitor<'_>) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL::from(true);
    }
    let Ok(title_len) = usize::try_from(unsafe { GetWindowTextLengthW(hwnd) }) else {
        return BOOL::from(true);
    };
    if title_len == 0 {
        return BOOL::from(true);
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let window = Window {
        hwnd,
        pid,
        title_len,
    };
    BOOL::from(visitor(&window).is_continue())
}

pub fn window_titles_by_pid() -> HashMap<u32, Vec<String>> {
    let mut titles: HashMap<u32, Vec<String>> = HashMap::new();
    for_each_window(|window| {
        let title = window.title();
        if !title.is_empty() {
            titles.entry(window.pid).or_default().push(title);
        }
        ControlFlow::Continue(())
    });
    titles
}

pub fn window_for_pid(pid: u32) -> Option<WindowHandle> {
    let mut found = None;
    for_each_window(|window| {
        if window.pid != pid {
            return ControlFlow::Continue(());
        }
        found = WindowHandle::from_hwnd(window.hwnd);
        ControlFlow::Break(())
    });
    found
}

pub fn set_window_title_for_pid(pid: u32, title: &str) -> bool {
    let title = title.trim();
    if pid == 0 || title.is_empty() {
        return false;
    }
    let wide = to_wide(title);
    let mut done = 0usize;
    for_each_window(|window| {
        if window.pid != pid {
            return ControlFlow::Continue(());
        }
        if window.title().trim() != title {
            let mut result = 0usize;
            unsafe {
                SendMessageTimeoutW(
                    window.hwnd,
                    WM_SETTEXT,
                    WPARAM(0),
                    LPARAM(wide.as_ptr() as isize),
                    SMTO_ABORTIFHUNG,
                    SET_TITLE_TIMEOUT_MS,
                    Some(&mut result),
                )
            };
        }
        done += 1;
        ControlFlow::Continue(())
    });
    done > 0
}

pub fn focus_window(window: WindowHandle) {
    let hwnd = window.hwnd();
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = BringWindowToTop(hwnd);
        let foreground = GetForegroundWindow();
        if foreground == hwnd {
            let _ = SetForegroundWindow(hwnd);
            return;
        }
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        if foreground_thread != 0 && foreground_thread != target_thread {
            let _ = AttachThreadInput(foreground_thread, target_thread, true);
            let _ = SetForegroundWindow(hwnd);
            let _ = AttachThreadInput(foreground_thread, target_thread, false);
        } else {
            let _ = SetForegroundWindow(hwnd);
        }
    }
}
