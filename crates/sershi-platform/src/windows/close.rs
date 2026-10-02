//! Graceful application close.
//!
//! Finds visible, unowned top-level windows whose process matches the
//! resolved application (by full image path, or by packaged AUMID) and posts
//! `WM_CLOSE`. A packaged (UWP) application such as Calculator draws inside
//! a frame window owned by `ApplicationFrameHost.exe`: that frame is matched
//! through its child window belonging to the application's process (Gate
//! 4.1.1) — the same request as clicking the window's × button, so the
//! application can ask to save. Processes are never terminated. SERSHI's own
//! windows are never touched.

use std::collections::HashMap;
use std::path::Path;

use sershi_core::apps::CloseSupport;
use sershi_core::ports::{ApplicationError, RunningState};
use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HWND, LPARAM, WPARAM};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::ProcessStatus::K32EnumProcesses;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GW_OWNER, GWL_EXSTYLE, GetWindow, GetWindowLongW,
    GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
    WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PWSTR};

use super::util::from_wide;

/// Reports (and, if `request_close`, closes) the application's windows.
pub fn state(close: &CloseSupport, request_close: bool) -> Result<RunningState, ApplicationError> {
    if matches!(close, CloseSupport::Unsupported) {
        return Err(ApplicationError::Unsupported);
    }
    let own = std::process::id();
    let mut cache: HashMap<u32, bool> = HashMap::new();
    let mut matched = |pid: u32| {
        *cache
            .entry(pid)
            .or_insert_with(|| process_matches(pid, close))
    };

    let top = top_level_windows();
    let mut windows: Vec<HWND> = top
        .iter()
        .filter(|(_, pid)| *pid != own && matched(*pid))
        .map(|(hwnd, _)| *hwnd)
        .collect();
    if packaged(close) {
        for (frame, pid) in &top {
            if *pid != own
                && is_frame_host(*pid)
                && child_pids(*frame)
                    .into_iter()
                    .any(|child| child != own && child != *pid && matched(child))
                && !windows.contains(frame)
            {
                windows.push(*frame);
            }
        }
    }

    if windows.is_empty() {
        let running = running_processes()
            .into_iter()
            .any(|pid| pid != own && matched(pid));
        return Ok(if running {
            RunningState::NoClosableWindow
        } else {
            RunningState::NotRunning
        });
    }
    if request_close {
        for hwnd in &windows {
            // SAFETY: posting WM_CLOSE to a window we enumerated; if it was
            // destroyed meanwhile the call simply fails.
            let _ = unsafe { PostMessageW(Some(*hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    }
    Ok(RunningState::Running {
        windows: u32::try_from(windows.len()).unwrap_or(u32::MAX),
    })
}

/// Visible, unowned, non-tool top-level windows with a title.
fn top_level_windows() -> Vec<(HWND, u32)> {
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the &mut Vec passed below, alive for the call.
        let out = unsafe { &mut *(lparam.0 as *mut Vec<(HWND, u32)>) };
        let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
        let owned = unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok_and(|o| !o.is_invalid());
        let tool = (unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32 & WS_EX_TOOLWINDOW.0) != 0;
        let titled = unsafe { GetWindowTextLengthW(hwnd) } > 0;
        if visible && !owned && !tool && titled {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            out.push((hwnd, pid));
        }
        BOOL(1)
    }
    let mut out: Vec<(HWND, u32)> = Vec::new();
    // SAFETY: the callback only writes into `out`, which outlives EnumWindows.
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut out as *mut _ as isize)) };
    out
}

fn packaged(close: &CloseSupport) -> bool {
    match close {
        CloseSupport::PackagedApp(_) => true,
        CloseSupport::Any(options) => options.iter().any(packaged),
        _ => false,
    }
}

/// Processes owning the child windows of `parent`.
fn child_pids(parent: HWND) -> Vec<u32> {
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the &mut Vec passed below, alive for the call.
        let out = unsafe { &mut *(lparam.0 as *mut Vec<u32>) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid != 0 && !out.contains(&pid) {
            out.push(pid);
        }
        BOOL(1)
    }
    let mut out: Vec<u32> = Vec::new();
    // SAFETY: the callback only writes into `out`, which outlives the call.
    let _ = unsafe {
        EnumChildWindows(
            Some(parent),
            Some(visit),
            LPARAM(&mut out as *mut _ as isize),
        )
    };
    out
}

/// Whether `pid` is the UWP frame host that draws packaged apps' windows.
fn is_frame_host(pid: u32) -> bool {
    // SAFETY: limited query access; the handle is closed before returning.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    let mut buf = [0u16; 1_024];
    let mut len = buf.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .is_ok();
    // SAFETY: handle from OpenProcess above.
    let _ = unsafe { CloseHandle(handle) };
    ok && from_wide(&buf)
        .to_ascii_lowercase()
        .ends_with(r"\applicationframehost.exe")
}

fn running_processes() -> Vec<u32> {
    let mut pids = vec![0u32; 4_096];
    let mut needed = 0u32;
    // SAFETY: bounded buffer, size passed in bytes.
    let ok = unsafe {
        K32EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * size_of::<u32>()) as u32,
            &mut needed,
        )
    }
    .as_bool();
    if !ok {
        return Vec::new();
    }
    pids.truncate(needed as usize / size_of::<u32>());
    pids.retain(|p| *p != 0);
    pids
}

fn process_matches(pid: u32, close: &CloseSupport) -> bool {
    // SAFETY: limited query access; the handle is closed before returning.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    let matches = |c: &CloseSupport| -> bool {
        match c {
            CloseSupport::ExecutablePath(expected) => {
                let mut buf = [0u16; 1_024];
                let mut len = buf.len() as u32;
                let ok = unsafe {
                    QueryFullProcessImageNameW(
                        handle,
                        PROCESS_NAME_WIN32,
                        PWSTR(buf.as_mut_ptr()),
                        &mut len,
                    )
                }
                .is_ok();
                ok && same_path(Path::new(&from_wide(&buf)), expected)
            }
            CloseSupport::PackagedApp(expected) => {
                let mut buf = [0u16; 512];
                let mut len = buf.len() as u32;
                let status = unsafe {
                    GetApplicationUserModelId(handle, &mut len, Some(PWSTR(buf.as_mut_ptr())))
                };
                status == ERROR_SUCCESS && from_wide(&buf).eq_ignore_ascii_case(expected)
            }
            CloseSupport::Any(_) | CloseSupport::Unsupported => false,
        }
    };
    let result = match close {
        CloseSupport::Any(options) => options.iter().any(matches),
        other => matches(other),
    };
    // SAFETY: handle from OpenProcess above.
    let _ = unsafe { CloseHandle(handle) };
    result
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
}
