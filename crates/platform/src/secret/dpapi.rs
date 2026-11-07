use std::slice;

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;

use crate::error::{PlatformError, Result};

pub(super) fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    let input = borrow(plain)?;
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    }
    .map_err(PlatformError::os("protect data"))?;
    Ok(take(output))
}

pub(super) fn unprotect(sealed: &[u8]) -> Result<Vec<u8>> {
    let input = borrow(sealed)?;
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    }
    .map_err(PlatformError::os("unprotect data"))?;
    Ok(take(output))
}

fn borrow(data: &[u8]) -> Result<CRYPT_INTEGER_BLOB> {
    let len = u32::try_from(data.len()).map_err(|_| PlatformError::TooLarge)?;
    Ok(CRYPT_INTEGER_BLOB {
        cbData: len,
        pbData: data.as_ptr().cast_mut(),
    })
}

fn take(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    if blob.pbData.is_null() {
        return Vec::new();
    }
    let bytes = unsafe { slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    unsafe { LocalFree(Some(HLOCAL(blob.pbData.cast()))) };
    bytes
}
