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
pub fn begin_move(window: &mut Window) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{
        HTCAPTION, PostMessageW, SC_MOVE, WM_SYSCOMMAND,
    };
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            let _ = ReleaseCapture();
            let _ = PostMessageW(
                Some(hwnd),
                WM_SYSCOMMAND,
                WPARAM((SC_MOVE | HTCAPTION) as usize),
                LPARAM(0),
            );
        }
    }
}

#[cfg(not(windows))]
pub fn begin_move(window: &mut Window) {
    window.start_window_move();
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

pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[cfg(windows)]
pub fn snapshot(window: &Window) -> Option<Snapshot> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    const CLIENT_FULL_CONTENT: u32 = 3;
    let hwnd = hwnd(window)?;
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect).ok()?;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return None;
        }
        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        let previous = SelectObject(memory, bitmap.into());
        let printed = PrintWindow(hwnd, memory, PRINT_WINDOW_FLAGS(CLIENT_FULL_CONTENT)).as_bool();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let lines = GetDIBits(
            memory,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        if !printed || lines == 0 {
            return None;
        }
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
        Some(Snapshot {
            width: width as u32,
            height: height as u32,
            pixels,
        })
    }
}

#[cfg(not(windows))]
pub fn snapshot(_window: &Window) -> Option<Snapshot> {
    None
}
