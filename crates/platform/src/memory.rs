#[cfg(windows)]
pub fn trim_memory() {
    use windows::Win32::System::Memory::{GetProcessHeap, HEAP_FLAGS, HeapCompact};
    use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};

    unsafe {
        if let Ok(heap) = GetProcessHeap() {
            HeapCompact(heap, HEAP_FLAGS(0));
        }
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(target_os = "linux")]
pub fn trim_memory() {
    unsafe { libc::malloc_trim(0) };
}
