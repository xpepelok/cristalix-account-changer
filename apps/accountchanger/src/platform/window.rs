use gpui::{App, Bounds, Pixels, Size, Window, px, size};

pub const PREFERRED: (f32, f32) = (1180.0, 820.0);

pub fn initial_bounds(cx: &App) -> Bounds<Pixels> {
    let (width, height) = PREFERRED;
    let (width, height) = match work_area(cx) {
        Some(area) => (
            width.min(f32::from(area.width)),
            height.min(f32::from(area.height)),
        ),
        None => (width, height),
    };
    Bounds::centered(None, size(px(width), px(height)), cx)
}

#[cfg(windows)]
fn work_area(_cx: &App) -> Option<Size<Pixels>> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    let mut rect = RECT::default();
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()?;
    }
    let scale = unsafe { GetDpiForSystem() }.max(96) as f32 / 96.0;
    Some(size(
        px((rect.right - rect.left) as f32 / scale),
        px((rect.bottom - rect.top) as f32 / scale),
    ))
}

#[cfg(not(windows))]
fn work_area(cx: &App) -> Option<Size<Pixels>> {
    cx.primary_display().map(|display| display.bounds().size)
}

#[cfg(windows)]
fn hwnd(window: &Window) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(windows::Win32::Foundation::HWND(
            handle.hwnd.get() as *mut core::ffi::c_void
        )),
        _ => None,
    }
}

#[cfg(windows)]
pub fn hide(window: &mut Window) {
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(not(windows))]
pub fn hide(window: &mut Window) {
    window.minimize_window();
}

#[cfg(windows)]
pub fn restore(window: &mut Window) {
    use windows::Win32::UI::WindowsAndMessaging::{
        IsIconic, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
    };
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            let _ = ShowWindow(
                hwnd,
                if IsIconic(hwnd).as_bool() {
                    SW_RESTORE
                } else {
                    SW_SHOW
                },
            );
            let _ = SetForegroundWindow(hwnd);
        }
    }
    window.activate_window();
}

#[cfg(not(windows))]
pub fn restore(window: &mut Window) {
    window.activate_window();
}

#[cfg(windows)]
pub fn toggle_maximize(window: &mut Window) {
    use windows::Win32::UI::WindowsAndMessaging::{
        IsZoomed, SW_MAXIMIZE, SW_RESTORE, ShowWindowAsync,
    };
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            let command = if IsZoomed(hwnd).as_bool() {
                SW_RESTORE
            } else {
                SW_MAXIMIZE
            };
            let _ = ShowWindowAsync(hwnd, command);
        }
    }
}

#[cfg(not(windows))]
pub fn toggle_maximize(window: &mut Window) {
    window.zoom_window();
}

#[cfg(windows)]
pub fn raise_topmost(window: &Window) {
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos,
    };
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            let _ = SetForegroundWindow(hwnd);
        }
    }
}
