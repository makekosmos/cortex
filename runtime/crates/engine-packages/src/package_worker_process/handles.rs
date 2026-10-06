#[cfg(windows)]
const CREATE_SUSPENDED: u32 = 0x0000_0004;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;
#[cfg(windows)]
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;

#[cfg(windows)]
struct ProcessHandle {
    handle: OwnedHandle,
    id: u32,
}
#[cfg(windows)]
struct OwnedHandle(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for OwnedHandle {}
#[cfg(windows)]
impl OwnedHandle {
    fn raw(&self) -> windows::Win32::Foundation::HANDLE {
        self.0
    }
}
#[cfg(windows)]
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(windows)]
unsafe impl Send for ProcessHandle {}

#[cfg(windows)]
struct JobHandle(OwnedHandle);
#[cfg(windows)]
unsafe impl Send for JobHandle {}
