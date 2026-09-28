//! Starting a resolved application.
//!
//! - Executables: `CreateProcessW` with the executable as the application
//!   name. No shell, no command interpreter, and Windows returns
//!   `ERROR_ELEVATION_REQUIRED` instead of silently prompting for elevation,
//!   so SERSHI never triggers UAC on its own.
//! - Packaged apps: `IApplicationActivationManager` by AUMID.
//! - Fixed URIs (`ms-settings:`) and installer-managed shortcuts:
//!   `ShellExecuteExW` with the "open" verb and no error UI.

use std::mem::size_of;
use std::path::Path;

use sershi_core::apps::LaunchTarget;
use sershi_core::ports::ApplicationError;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ELEVATION_REQUIRED, ERROR_FILE_NOT_FOUND,
    ERROR_PATH_NOT_FOUND,
};
use windows::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{Error, HRESULT, PCWSTR, PWSTR, w};

use super::packaged;
use super::util::wide;

pub fn launch(target: &LaunchTarget) -> Result<(), ApplicationError> {
    match target {
        LaunchTarget::Executable {
            path,
            arguments,
            working_dir,
        } => {
            if !path.is_file() {
                return Err(ApplicationError::TargetMissing);
            }
            create_process(path, arguments.as_deref(), working_dir.as_deref())
        }
        LaunchTarget::PackagedApp { aumid } => packaged::activate(aumid),
        LaunchTarget::ShellUri { uri } => shell_open(uri),
        LaunchTarget::Shortcut { path } => {
            if !path.is_file() {
                return Err(ApplicationError::TargetMissing);
            }
            shell_open(path.as_os_str())
        }
    }
}

fn create_process(
    exe: &Path,
    arguments: Option<&str>,
    working_dir: Option<&Path>,
) -> Result<(), ApplicationError> {
    let application = wide(exe);
    // argv[0] is the quoted executable; arguments come only from the
    // shortcut that registered the application, never from input.
    let mut command_line = format!("\"{}\"", exe.display());
    if let Some(args) = arguments {
        command_line.push(' ');
        command_line.push_str(args);
    }
    let mut command_line = wide(command_line);
    let directory = working_dir
        .filter(|d| d.is_dir())
        .or_else(|| exe.parent())
        .map(wide);
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    // SAFETY: all strings are NUL-terminated buffers that outlive the call;
    // the command-line buffer is mutable as CreateProcessW requires; the
    // returned handles are closed below.
    let result = unsafe {
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_UNICODE_ENVIRONMENT,
            None,
            directory
                .as_ref()
                .map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
            &startup,
            &mut info,
        )
    };
    result.map_err(|e| map_error(&e))?;
    // SAFETY: handles returned by a successful CreateProcessW.
    unsafe {
        let _ = CloseHandle(info.hThread);
        let _ = CloseHandle(info.hProcess);
    }
    Ok(())
}

fn shell_open(file: impl AsRef<std::ffi::OsStr>) -> Result<(), ApplicationError> {
    let file = wide(file);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialized and its strings outlive the call.
    unsafe { ShellExecuteExW(&mut info) }.map_err(|e| map_error(&e))
}

/// Maps Win32 failures to SERSHI's categories. Raw codes stay internal.
pub fn map_error(error: &Error) -> ApplicationError {
    let code = error.code();
    if code == HRESULT::from_win32(ERROR_ELEVATION_REQUIRED.0) {
        ApplicationError::ElevationRequired
    } else if code == HRESULT::from_win32(ERROR_ACCESS_DENIED.0) {
        ApplicationError::AccessDenied
    } else if code == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
        || code == HRESULT::from_win32(ERROR_PATH_NOT_FOUND.0)
    {
        ApplicationError::TargetMissing
    } else {
        ApplicationError::Failed(format!("{code:?}"))
    }
}
