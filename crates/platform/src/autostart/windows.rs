use std::env;
use std::mem::size_of_val;

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{PCWSTR, w};

use crate::error::{PlatformError, Result};
use crate::win::to_wide;

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const VALUE_NAME: PCWSTR = w!("AccountChanger");
const EMPTY_STRING_BYTES: u32 = 2;

pub fn autostart_enabled() -> bool {
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE_NAME,
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND,
            None,
            None,
            Some(&mut size),
        )
    };
    status == ERROR_SUCCESS && size > EMPTY_STRING_BYTES
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    if !enabled {
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        return status
            .ok()
            .map_err(PlatformError::os("remove autostart entry"));
    }
    let exe = env::current_exe()?;
    let command = to_wide(&format!("\"{}\"", exe.display()));
    let bytes =
        u32::try_from(size_of_val(command.as_slice())).map_err(|_| PlatformError::TooLarge)?;
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE_NAME,
            REG_SZ.0,
            Some(command.as_ptr().cast()),
            bytes,
        )
    }
    .ok()
    .map_err(PlatformError::os("write autostart entry"))
}
