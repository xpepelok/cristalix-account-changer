use windows::Win32::Foundation::{CloseHandle, HANDLE};

pub(crate) struct OwnedHandle(pub(crate) HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(crate) fn from_wide(units: &[u16]) -> String {
    let end = units
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

pub(crate) fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
