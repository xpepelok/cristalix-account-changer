use std::ffi::c_void;
use std::mem::{MaybeUninit, offset_of, size_of};
use std::ops::ControlFlow;

use windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessBasicInformation};
use windows::Win32::Foundation::{HANDLE, LPARAM, UNICODE_STRING, WPARAM};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, PEB, PROCESS_BASIC_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE, PROCESS_VM_READ, RTL_USER_PROCESS_PARAMETERS, TerminateProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

use super::ProcessInfo;
use crate::win::{OwnedHandle, from_wide};
use crate::window::for_each_window;

pub fn processes() -> Vec<ProcessInfo> {
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    let snapshot = OwnedHandle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut out = Vec::with_capacity(256);
    let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_ok();
    while more {
        out.push(ProcessInfo {
            pid: entry.th32ProcessID,
            ppid: entry.th32ParentProcessID,
            name: from_wide(&entry.szExeFile),
        });
        more = unsafe { Process32NextW(snapshot.0, &mut entry) }.is_ok();
    }
    out
}

pub fn command_line(pid: u32) -> Option<String> {
    let access = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ;
    let process = OwnedHandle(unsafe { OpenProcess(access, false, pid) }.ok()?);
    let mut basic = PROCESS_BASIC_INFORMATION::default();
    let mut written = 0u32;
    let status = unsafe {
        NtQueryInformationProcess(
            process.0,
            ProcessBasicInformation,
            (&raw mut basic).cast(),
            size_of::<PROCESS_BASIC_INFORMATION>() as u32,
            &mut written,
        )
    };
    if status.is_err() || basic.PebBaseAddress.is_null() {
        return None;
    }
    let params: *mut RTL_USER_PROCESS_PARAMETERS = read_remote(
        process.0,
        basic
            .PebBaseAddress
            .cast::<c_void>()
            .wrapping_byte_add(offset_of!(PEB, ProcessParameters)),
    )?;
    if params.is_null() {
        return None;
    }
    let line: UNICODE_STRING = read_remote(
        process.0,
        params
            .cast::<c_void>()
            .wrapping_byte_add(offset_of!(RTL_USER_PROCESS_PARAMETERS, CommandLine)),
    )?;
    let units = usize::from(line.Length) / 2;
    if units == 0 || line.Buffer.is_null() {
        return Some(String::new());
    }
    let mut buffer = vec![0u16; units];
    unsafe {
        ReadProcessMemory(
            process.0,
            line.Buffer.0.cast(),
            buffer.as_mut_ptr().cast(),
            units * 2,
            None,
        )
    }
    .ok()?;
    Some(String::from_utf16_lossy(&buffer).trim().to_owned())
}

fn read_remote<T: Copy>(process: HANDLE, address: *const c_void) -> Option<T> {
    let mut value = MaybeUninit::<T>::uninit();
    unsafe {
        ReadProcessMemory(
            process,
            address,
            value.as_mut_ptr().cast(),
            size_of::<T>(),
            None,
        )
    }
    .ok()?;
    Some(unsafe { value.assume_init() })
}

pub fn close_game(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let mut posted = 0usize;
    for_each_window(|window| {
        if window.pid == pid
            && unsafe { PostMessageW(Some(window.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_ok()
        {
            posted += 1;
        }
        ControlFlow::Continue(())
    });
    if posted > 0 {
        return true;
    }
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }) else {
        return false;
    };
    let handle = OwnedHandle(handle);
    unsafe { TerminateProcess(handle.0, 1) }.is_ok()
}
