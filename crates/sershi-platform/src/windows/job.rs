//! A process-lifetime job object for SERSHI's helper processes (the local
//! semantic engine): when SERSHI exits or crashes, Windows closes the job
//! handle and ends every process in it, so no helper outlives SERSHI.

use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::sync::OnceLock;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

/// The job handle as an integer (a `HANDLE` is not `Sync`). Never closed:
/// it is closed by Windows when SERSHI's process ends, which is the point.
fn job() -> Option<isize> {
    static JOB: OnceLock<Option<isize>> = OnceLock::new();
    *JOB.get_or_init(|| {
        // SAFETY: creates an unnamed job object owned by this process.
        let job = unsafe { CreateJobObjectW(None, None) }.ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `info` is a valid, correctly sized structure for this
        // information class, alive for the duration of the call.
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&info).cast(),
                u32::try_from(std::mem::size_of_val(&info)).ok()?,
            )
        }
        .ok()?;
        Some(job.0 as isize)
    })
}

/// Puts a child process in SERSHI's job (best effort: if it fails, the
/// helper still ends when its stdin closes, i.e. when SERSHI exits).
pub fn contain(child: &Child) {
    let Some(job) = job() else {
        return;
    };
    // SAFETY: both handles are valid for the duration of the call: the job
    // lives for the process lifetime and `child` owns its process handle.
    let _ = unsafe {
        AssignProcessToJobObject(
            HANDLE(job as *mut std::ffi::c_void),
            HANDLE(child.as_raw_handle()),
        )
    };
}
