//! Free disk space, so a large model download never starts on a volume
//! that cannot hold it (Prompt 4; Gate 3C filled a nearly full C: drive).

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::core::PCWSTR;

/// Bytes available to this user on the volume holding `dir`.
pub fn free_bytes(dir: &Path) -> Option<u64> {
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0u64;
    // SAFETY: `wide` is a NUL-terminated UTF-16 path alive for the call;
    // the out-pointer refers to a live local.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&raw mut available), None, None) }
        .ok()?;
    Some(available)
}
