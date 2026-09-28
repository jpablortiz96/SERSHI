//! Small helpers shared by the Windows modules.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::core::PWSTR;

/// Initializes COM for the current thread for the guard's lifetime.
pub struct ComApartment {
    owned: bool,
}

impl ComApartment {
    pub fn init() -> Self {
        // SAFETY: plain COM initialization; balanced by `Drop` only when it
        // succeeded (S_OK or S_FALSE). RPC_E_CHANGED_MODE means the thread is
        // already initialized differently, which is fine to use as-is.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Self { owned: hr.is_ok() }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: balances the successful CoInitializeEx above.
            unsafe { CoUninitialize() };
        }
    }
}

/// Null-terminated UTF-16 for Win32.
pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}

/// Reads a UTF-16 buffer up to its first NUL.
pub fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Takes ownership of a COM-allocated string and frees it.
pub fn take_pwstr(p: PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is a valid, NUL-terminated string allocated by the COM task
    // allocator (documented for the APIs we call); it is freed exactly once.
    let value = unsafe { p.to_string().ok() };
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    value
}

/// Runs `f` on its own thread and gives up waiting after `timeout`. The
/// thread is detached on timeout; the caller gets `None`.
pub fn with_timeout<T: Send + 'static>(
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    let spawned = thread::Builder::new()
        .name("sershi-native".to_owned())
        .spawn(move || {
            let _ = tx.send(f());
        });
    if spawned.is_err() {
        return None;
    }
    rx.recv_timeout(timeout).ok()
}
